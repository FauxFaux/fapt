use std::fs;
use std::path::Path;

use reqwest;

use classic_sources_list::Entry;
use release;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>>(sources_entries: &[Entry], cache: P) -> Result<()> {
    // TODO: keyring paths

    let client = reqwest::Client::new();
    let lists_dir = cache.as_ref().join("lists");
    fs::create_dir_all(&lists_dir).chain_err(|| {
        format!("creating cache directory: {:?}", lists_dir)
    })?;

    let releases = release::load(&sources_entries, &lists_dir).chain_err(
        || "loading releases",
    )?;

    lists::download_files(&client, &lists_dir, &releases)?;

    Ok(())
}
