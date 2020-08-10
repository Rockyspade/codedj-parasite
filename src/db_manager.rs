use std::sync::*;
use std::fs::*;

use crate::*;

/** R/W manager for the downloader database to be used by the downloade & friends.
 
    
 */
pub struct DatabaseManager {
    // root folder where all the data lives
    root_ : String, 
    // set of live urls for each active project so that we can easily check for project duplicites
    // TODO in the future, we also need set of dead urls
    // and we really only need to build this lazily when needed IMHO
    live_urls_ : Mutex<HashSet<String>>,

    // number of projects (dead and alive), used for generating new project ids...
    num_projects_ : Mutex<u64>,

    /* User email to user id mapping and file to which new mappings or updates should be written
     */
    user_ids_ : Mutex<HashMap<String, UserId>>,
    user_ids_file_ : Mutex<File>,
    user_records_file_ : Mutex<File>,

    /* SHA1 to commit id mapping and a file to which any new mappings should be written and a file to which new commit records are written. 

       TODO For now the API obtains locks every time a single commit is written, which is not super effective, this could be revisited in the future.
     */
    commit_ids_ : Mutex<HashMap<git2::Oid, CommitId>>,
    commit_ids_file_ : Mutex<File>,
    commit_records_file_ : Mutex<File>,
    commit_parents_file_ : Mutex<File>,
}

impl DatabaseManager {

    /** Creates new database manager and initializes its database in the given folder.
     
        If the folder exists, all its contents is deleted first. 
     */
    pub fn initialize_new(root_folder : String) -> DatabaseManager {
        // initialize the folder
        if std::path::Path::new(& root_folder).exists() {
            std::fs::remove_dir_all(& root_folder).unwrap();
        }
        std::fs::create_dir_all(& root_folder).unwrap();
        // create the necessary files
        let user_ids_file = format!("{}/user_ids.csv", root_folder);
        {
            let mut f = File::create(& user_ids_file).unwrap();
            writeln!(& mut f, "email,id").unwrap();
        }
        let user_records_file = format!("{}/user_records.csv", root_folder);
        {
            let mut f = File::create(& user_records_file).unwrap();
            writeln!(& mut f, "time,id,name,source").unwrap();
        }
        let commit_ids_file = format!("{}/commit_ids.csv", root_folder);
        {
            let mut f = File::create(& commit_ids_file).unwrap();
            writeln!(& mut f, "hash,id").unwrap();
        }
        let commit_records_file = format!("{}/commit_records.csv", root_folder);
        {
            let mut f = File::create(& commit_records_file).unwrap();
            writeln!(& mut f, "time,id,committerId,committerTime,authorId,authorTime,source").unwrap();
        }
        let commit_parents_file = format!("{}/commit_parents.csv", root_folder);
        {
            let mut f = File::create(& commit_parents_file).unwrap();
            writeln!(& mut f, "time,commitId,parentId").unwrap();
        }
        // create the manager and return it
        let result = DatabaseManager{
            root_ : root_folder,
            live_urls_ : Mutex::new(HashSet::new()),
            num_projects_ : Mutex::new(0),

            user_ids_ : Mutex::new(HashMap::new()),
            user_ids_file_ : Mutex::new(OpenOptions::new().append(true).open(& user_ids_file).unwrap()), 
            user_records_file_ : Mutex::new(OpenOptions::new().append(true).open(& user_records_file).unwrap()),

            commit_ids_ : Mutex::new(HashMap::new()),
            commit_ids_file_ : Mutex::new(OpenOptions::new().append(true).open(& commit_ids_file).unwrap()), 
            commit_records_file_ : Mutex::new(OpenOptions::new().append(true).open(& commit_records_file).unwrap()),
            commit_parents_file_ : Mutex::new(OpenOptions::new().append(true).open(& commit_parents_file).unwrap()),
        };
        // commit the 0 created projects to begin with
        result.commit_created_projects();
        // and return the new database manager
        return result;
    }

    /** Creates database manager from existing database folder.
     */
    pub fn from(root_folder : String) -> DatabaseManager {
        //let num_projects = Self::get_num_projects(& root_folder);
        // load user ids mapping
        /*
        let mut user_ids = HashMap::<String,UserId>::new();
        let user_ids_file = format!("{}/user_ids.csv", root_folder);
        {
            let mut reader = csv::ReaderBuilder::new().has_headers(true).double_quote(false).escape(Some(b'\\')).from_path(& user_ids_file).unwrap();
            println!("Loading user ids...");
            for x in reader.records() {
                let record = x.unwrap();
                let email = String::from(& record[0]);
                let user_id = record[1].parse::<u64>().unwrap() as UserId;
                user_ids.insert(email, user_id);
            }
            println!("    {} users loaded", user_ids.len());
        }
        */
        // load commit ids mapping
        // and so on...
        unimplemented!();
    }

    /** Creates new project with given url and source.
     
        If the url is new, returns the id assigned to the project, ortherwise returns None. The project log is initialized with init message of the appropriate url and source.  

        Note that the function does not commit the changes to the database. 
     */
    pub fn add_project(& self, url : String, source : Source) -> Option<ProjectId> {
        let mut live_urls = self.live_urls_.lock().unwrap(); // we lock for too long, but not care now
        // don't know how to do this on single lookup in rust yet
        if live_urls.contains(& url) {
            return None;
        }
        // get the project id
        let mut num_projects = self.num_projects_.lock().unwrap();
        let id = *num_projects as ProjectId;
        // get the project folder and create it 
        let project_folder = Self::get_project_log_folder(& self.root_, id);
        std::fs::create_dir_all(& project_folder).unwrap();
        // initialize the log for the project
        {
            let mut project_log = self.get_project_log(id);
            project_log.add(record::ProjectLogEntry::init(source, url.clone()));
            project_log.create_and_save();
        }
        // now that the log is ok, increment total number of projects, add the live url and return the id
        *num_projects += 1;
        live_urls.insert(url);
        return Some(id);
    }

    /** Commits the total number of projects which makes them reachable. 
     
        Technically this could happen after each new project is created, but that is too prohibitive so it is the responsibility of the code that adds projects to actually commit the number once the projects are created. 
     */
    pub fn commit_created_projects(& self) {
        let num_projects = self.num_projects_.lock().unwrap();
        let mut f = File::create(format!("{}/num_projects.csv", self.root_)).unwrap();
        write!(& mut f, "numProjects\n{}\n", num_projects).unwrap();
    }

    /** Returns project log corresponding to given project.
     
        It is assumed that the project already exists. The log is not read.
     */ 
    pub fn get_project_log(& self, id : ProjectId) -> record::ProjectLog {
        return record::ProjectLog{
            filename_ : Self::get_project_log_file(& self.root_, id),
            entries_ : Vec::new(),
        };
    }

    // TODO read project log? 

    /** Returns existing user id, or creates new user from given data.
     
        
     */
    pub fn get_or_create_user(& self, email : & str, name : & str, source: Source) -> UserId {
        let mut user_ids = self.user_ids_.lock().unwrap();
        if let Some(id) = user_ids.get(email) {
            return *id;
        } else {
            let id = user_ids.len() as UserId;
            user_ids.insert(String::from(email), id);
            // first store the email to id mapping
            {
                let mut user_ids_file = self.user_ids_file_.lock().unwrap();
                writeln!(user_ids_file, "\"{}\",{}", String::from(email), id).unwrap();
            }
            // then store the actual user record
            {
                let mut user_records_file = self.user_records_file_.lock().unwrap();
                record::User::new(id, String::from(name), source).to_csv(& mut user_records_file).unwrap();
            }
            return id;
        }
    }

    /** Returns id for given commit if the commit exists in the database. 
        
     */
    pub fn get_commit_id(& self, hash : git2::Oid) -> Option<CommitId> {
        let commit_ids = self.commit_ids_.lock().unwrap();
        match commit_ids.get(& hash) {
            Some(id) => {
                return Some(*id);
            },
            _ => {
                return None;
            }
        }
    }

    pub fn create_commit(& self, hash: git2::Oid, committer_id : UserId, committer_time : u64, author_id : UserId, author_time : u64, source : Source) -> CommitId {
        let mut commit_ids = self.commit_ids_.lock().unwrap();
        let id = commit_ids.len() as CommitId;
        commit_ids.insert(hash, id);
        // write the hash to id mapping
        {
            let mut commit_ids_file = self.commit_ids_file_.lock().unwrap();
            writeln!(commit_ids_file, "{},{}", hash, id).unwrap();
        }
        // write the commit record
        {
            let mut commit_records_file = self.commit_records_file_.lock().unwrap();
            record::Commit::new(id, committer_id, committer_time, author_id, author_time, source).to_csv(& mut commit_records_file).unwrap();
        }
        return id;
    }

    pub fn append_commit_parents_record(& self, iter : & mut dyn std::iter::Iterator<Item = &(CommitId, CommitId)>) {
        let mut commit_parents_file = self.commit_parents_file_.lock().unwrap();
        let t = helpers::now();
        for (commit_id, parent_id) in iter {
            writeln!(commit_parents_file, "{},{},{}", t, commit_id, parent_id).unwrap();
        }
    }

    // bookkeeping & stuff

    /** Returns the log file for given project id. 
     
        
     */
    pub fn get_project_log_file(root : & str, id : ProjectId) -> String {
        return format!("{}/projects/{}/{}/{}.csv", root, id / 1000000, id % 1000, id);
    }

    /** Returns only the folder where the project log should exist so that we can ensure its presence. 
     */
    fn get_project_log_folder(root : & str, id : ProjectId) -> String {
        return format!("{}/projects/{}/{}", root, id / 1000000, id % 1000);
    }

    pub fn get_num_projects(root : & str) -> u64 {
        let mut reader = csv::ReaderBuilder::new().has_headers(true).double_quote(false).escape(Some(b'\\')).from_path(format!("{}/num_projects.csv", root)).unwrap();
        for x in reader.records() {
            let record = x.unwrap();
            return record[0].parse::<u64>().unwrap();
        }
        panic!("Invalid number of projects format.");
    }

    pub fn get_commit_ids(root : & str) -> HashMap<git2::Oid, CommitId> {
        let mut result = HashMap::<git2::Oid,CommitId>::new();
        let mut reader = csv::ReaderBuilder::new().has_headers(true).double_quote(false).escape(Some(b'\\')).from_path(format!("{}/commit_ids.csv", root)).unwrap();
        for x in reader.records() {
            let record = x.unwrap();
            let hash = git2::Oid::from_str(& record[0]).unwrap();
            let id = record[1].parse::<u64>().unwrap() as CommitId;
            result.insert(hash, id);
        }
        return result;
    }

}