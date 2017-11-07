use std::path::Path;

use reqwest;
use tempdir::TempDir;

use classic_sources_list;
use fetch;
use release;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>, Q: AsRef<Path>>(sources_list_path: P, cache: Q) -> Result<()> {
    // TODO: sources.list.d
    // TODO: keyring paths

    let client = reqwest::Client::new();
    let lists_dir = cache.as_ref().join("lists");

    let sources_entries = classic_sources_list::load(&sources_list_path)
        .chain_err(|| format!("loading sources.list: {:?}", sources_list_path.as_ref()))?;

    let releases = release::load(&sources_entries, &lists_dir)
        .chain_err(|| "loading releases")?;

    let files = lists::find_files(&releases)
        .chain_err(|| "filtering releases")?;

    let temp_dir = TempDir::new("fapt-lists")
        .chain_err(|| "creating temporary directory")?;

    let downloads: Vec<fetch::Download> = files
        .iter()
        .filter_map(|list| {
            let local_name = list.local_name();

            match lists_dir.join(&local_name).exists() {
                true => None,
                false => Some(fetch::Download::from_to(
                    list.url.clone(),
                    temp_dir.as_ref().join(local_name),
                )),
            }
        })
        .collect();

    fetch::fetch(&client, &downloads)?;

    Ok(())
}
