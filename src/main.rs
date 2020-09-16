use std::collections::*;
use dcd::*;

mod db;
mod datastore;
mod updater;
mod repo_updater;
mod records;
mod helpers;

use datastore::*;
use updater::*;

fn main() {
    println!("DejaCode Downloader mark II");
    let args : Vec<String> = std::env::args().collect();
    let mut i = 1;
    if args.len() <= i {
        help()
    }
    let mut wd = String::from(std::env::current_dir().unwrap().to_str().unwrap());
    if args[i].starts_with("-o=") {
        wd = args[i][3..].to_string();
        i += 1;
    }
    if args.len() <= i {
        help()
    }
    let cmd = & args[i];
    i += 1;
    match cmd.as_str() {
        "init" => dcd_init(& wd, & args[i..]),
        "add" => dcd_add(& wd, & args[i..]),
        "update" => dcd_update(& wd, & args[i..]),
        "export" => dcd_export(& wd, & args[i..]),
        &_ => help(),
    }
}

/** Initializes the datastore in current directory.  
 */
fn dcd_init(working_dir : & str, args : & [String]) {
    // clear and create the working directory
    let wd_path = std::path::Path::new(working_dir);
    if wd_path.exists() {
        std::fs::remove_dir_all(&wd_path).unwrap();
    }
    std::fs::create_dir_all(&wd_path).unwrap();
    // create the datastore and initialize the basic values
    let ds = Datastore::from(working_dir);
    println!("Initializing new repository with common values...");
    ds.hashes.lock().unwrap().get_or_create(& git2::Oid::zero());
    println!("    hash 0");
}

/** Adds projects from given file or a single url project to the datastore. 

    For now, for project to be added, it must have unique url across all of the known urls, including the dead ones. This is correct for most cases, but one can imagive a project being created, then developed, then deleted and then a project of the same name, but different one being created as well. Or even moved and then old name reused.

    TODO how to actually handle this?
 */
fn dcd_add(working_dir : & str, args : & [String]) {
    if args.len() < 1 {
        help();
    }
    let ds = Datastore::from(working_dir);
    println!("Loading known project urls...");
    let mut urls = HashSet::<String>::new();
    for (id, url) in ds.project_urls.lock().unwrap().all_iter() {
        urls.insert(url);
    }
    println!("    urls: {}", urls.len());
    // now go through all arguments and see if they can be added
    for arg in args {
        if arg.starts_with("https://") {
            println!("Adding project {}", arg);
            if urls.contains(arg) {
                println!("    already exists");
            } else {
                println!("    added as id: {}", ds.add_project(arg));
                urls.insert(arg.to_owned());
            }
        } else if arg.ends_with(".csv") {
            add_projects_from_csv(arg, & ds, & mut urls);
        } else {
            println!("Unrecognized project file or url format: {}", arg);
            help();
        }
    }
}

/** Given a csv file determines if it contains headers or not and determines the column that contains urls and adds projects from these urls to the datastore. 
 
    Column contains url if it starts with `https://`. Only one column can contain url for the csv to be parsed correctly and the column must stay the same for the entire file. 
 */
fn add_projects_from_csv(filename : & str, ds : & Datastore, urls : & mut HashSet<String>) {
    println!("Adding projects from csv file {}", filename);
    let mut records = 0;
    let mut added = 0;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .double_quote(false)
        .escape(Some(b'\\'))
        .from_path(filename).unwrap();
    let headers = reader.headers().unwrap();
    let mut col_id = if let Some(id) = analyze_csv_row(& headers) {
        records += 1;
        let url = String::from(& headers[id]);
        if ! urls.contains(& url) {
            ds.add_project(& url);
            urls.insert(url);
            added += 1;
        }
        id
    } else {
        std::usize::MAX
    };
    for x in reader.records() {
        let record = x.unwrap();
        if col_id == std::usize::MAX {
            if let Some(id) = analyze_csv_row(& record) {
                col_id = id;
            } else {
                println!("Cannot determine which column contains git urls.");
                help();
            }
        }
        records += 1;
        let url = String::from(& record[col_id]);
        if ! urls.contains(& url) {
            ds.add_project(& url);
            urls.insert(url);
            added += 1;
        }
    }
    println!("    {} records", records);
    println!("    {} projects already exist", records - added);
    println!("    {} projects added", added);
}

fn analyze_csv_row(row : & csv::StringRecord) -> Option<usize> {
    let mut i : usize = 0;
    let mut result : usize = std::usize::MAX;
    for x in row {
        if x.starts_with("https://") {
            // there are multiple indices that could be urls, so we can't determine 
            if result != std::usize::MAX {
                return None;
            }
            result = i;
        }
        i += 1;
    }
    if result != std::usize::MAX {
        return Some(result);
    } else {
        return None;
    }
}

/** Runs the incremental updater. 
 
    Creates the updater and starts the continuous update of the projects. 
 */ 
fn dcd_update(working_dir : & str, args : & [String]) {
    let mut updater = Updater::new(Datastore::from(working_dir));
    updater.run();

}

fn dcd_export(working_dir : & str, args : & [String]) {
    let dsview = DatastoreView::new(working_dir, helpers::now());
    for (id, commit) in dsview.commits() {

    }

}




fn help() {
    println!("Usage:");

    std::process::exit(-1);
}