use std::path::Path;

use classic_sources_list;
use release;

use errors::*;

pub fn update<P: AsRef<Path>, Q: AsRef<Path>>(sources_list_path: P, cache: Q) -> Result<()> {
    // TODO: sources.list.d
    // TODO: keyring paths
    let sources_entries = classic_sources_list::load(sources_list_path)?;
    let req_releases = release::releases(&sources_entries)?;

    let lists_dir = cache.as_ref().join("lists");
    let release_files = release::download_releases(
        lists_dir,
        &req_releases,
        &["/usr/share/keyrings/debian-archive-keyring.gpg"],
    )?;

    let parsed_files: Vec<release::ReleaseFile> =
        release_files
            .iter()
            .map(release::parse_release_file)
            .collect::<Result<Vec<release::ReleaseFile>>>()?;

    for file in parsed_files {
        println!("\n\n{:?}", file);
    }

    Ok(())
}
