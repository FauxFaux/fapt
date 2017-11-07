use std::path::Path;

use reqwest;

use classic_sources_list;
use fetch;
use release;
use lists;

use errors::*;

pub fn update<P: AsRef<Path>, Q: AsRef<Path>>(sources_list_path: P, cache: Q) -> Result<()> {
    // TODO: sources.list.d
    // TODO: keyring paths
    let sources_entries = classic_sources_list::load(sources_list_path)?;
    let req_releases = release::interpret(&sources_entries)?;

    let known_releases: Vec<&release::RequestedRelease> = req_releases.keys().collect();

    let lists_dir = cache.as_ref().join("lists");
    let release_files = release::download_releases(
        lists_dir,
        &known_releases,
        &["/usr/share/keyrings/debian-archive-keyring.gpg"],
    )?;

    let parsed_files: Vec<release::ReleaseFile> =
        release_files
            .iter()
            .map(release::parse_release_file)
            .collect::<Result<Vec<release::ReleaseFile>>>()?;

    let client = reqwest::Client::new();

    let mut downloads = Vec::new();

    for (file, req) in parsed_files.into_iter().zip(known_releases) {
        let req: &release::RequestedRelease = req;
        let dists = req.dists()?;

        let entries = req_releases.get(req).expect(
            "everything should still line up",
        );

        for entry in entries {
            for component in &entry.components {
                let list = lists::find_file(
                    &file.contents,
                    &if entry.src {
                        format!("{}/source/Sources", component)
                    } else {
                        // TODO: arch
                        format!("{}/binary-amd64/Packages", component)
                    },
                )?;

                downloads.push(fetch::Download::from_to(dists.join(&list.path)?, list.path));
            }
        }
    }

    fetch::fetch(&client, &downloads)?;

    Ok(())
}
