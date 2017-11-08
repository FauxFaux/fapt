use std::fs;
use std::io;
use std::path::Path;

use reqwest;
use serde_json;

use classic_sources_list::Entry;
use release;
use rfc822;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>>(sources_entries: &[Entry], lists_dir: P) -> Result<()> {
    // TODO: keyring paths

    let client = reqwest::Client::new();

    let releases = release::load(&sources_entries, &lists_dir).chain_err(
        || "loading releases",
    )?;

    lists::download_files(&client, &lists_dir, &releases)?;

    Ok(())
}

pub fn export<P: AsRef<Path>>(sources_entries: &[Entry], lists_dir: P) -> Result<()> {
    let releases: Vec<release::Release> = release::load(&sources_entries, &lists_dir).chain_err(
        || "loading releases",
    )?;

    let client = reqwest::Client::new();

    lists::download_files(&client, &lists_dir, &releases)?;

    let lists = lists::find_files(&releases)?;
    for list in lists {
        let file = fs::File::open(lists_dir.as_ref().join(list.local_name()))?;
        for section in rfc822::Section::new(file) {
            let section = String::from_utf8(section?)?;
            let map = rfc822::map(&section).chain_err(|| {
                format!("scanning {:?}", list.local_name())
            })?;
            serde_json::to_writer(io::stdout(), &map)?;
            println!();
        }
    }

    Ok(())
}
