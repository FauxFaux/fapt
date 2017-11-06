use std::path::Path;

use classic_sources_list;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>, Q: AsRef<Path>>(sources_list_path: P, cache: Q) -> Result<()> {
    // TODO: sources.list.d
    let sources_entries = classic_sources_list::load(sources_list_path)?;
    let req_releases = lists::releases(&sources_entries)?;

    let lists_dir = cache.as_ref().join("lists");
    let release_files = lists::download_releases(lists_dir, &req_releases)?;


    Ok(())
}
