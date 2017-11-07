use std::path::Path;

use reqwest;

use classic_sources_list;
use release;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>, Q: AsRef<Path>>(sources_list_path: P, cache: Q) -> Result<()> {
    // TODO: sources.list.d
    // TODO: keyring paths

    let client = reqwest::Client::new();
    let lists_dir = cache.as_ref().join("lists");

    let sources_entries = classic_sources_list::load(&sources_list_path).chain_err(
        || {
            format!("loading sources.list: {:?}", sources_list_path.as_ref())
        },
    )?;

    let releases = release::load(&sources_entries, &lists_dir).chain_err(
        || "loading releases",
    )?;

    lists::download_files(&client, &lists_dir, &releases)?;

    Ok(())
}
