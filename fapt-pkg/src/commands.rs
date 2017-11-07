use std::fs;
use std::io;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use flate2;
use reqwest;
use tempdir::TempDir;
use tempfile_fast::persistable_tempfile_in;

use classic_sources_list;
use checksum;
use fetch;
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

    let lists = lists::find_files(&releases).chain_err(
        || "filtering releases",
    )?;

    let temp_dir = TempDir::new_in(&lists_dir, ".fapt-lists").chain_err(
        || "creating temporary directory",
    )?;

    let downloads: Vec<fetch::Download> = lists
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

    fetch::fetch(&client, &downloads).chain_err(
        || "downloading listed files",
    )?;

    for list in lists {
        let local_name = list.local_name();
        let destination_path = lists_dir.join(&local_name);
        if destination_path.exists() {
            continue;
        }

        let temp_path = temp_dir.as_ref().join(&local_name);
        let mut temp = fs::File::open(&temp_path).chain_err(
            || "opening a temp file we just downloaded",
        )?;

        checksum::validate(&mut temp, list.compressed_hashes)
            .chain_err(|| format!("validating downloaded file: {:?}", temp_path))?;

        match list.codec {
            lists::Compression::None => fs::rename(temp_path, destination_path)?,
            lists::Compression::Gz => {
                let mut uncompressed_temp = persistable_tempfile_in(&lists_dir)?;
                temp.seek(SeekFrom::Start(0))?;

                io::copy(
                    &mut flate2::bufread::GzDecoder::new(io::BufReader::new(&mut temp))?,
                    uncompressed_temp.as_mut(),
                ).chain_err(|| format!("decomressing {:?}", temp_path))?;
                uncompressed_temp.as_mut().seek(SeekFrom::Start(0))?;
                checksum::validate(uncompressed_temp.as_mut(), list.decompressed_hashes)
                    .chain_err(|| "validating decompressed file")?;
                uncompressed_temp
                    .persist_noclobber(destination_path)
                    .chain_err(|| "storing decompressed file")?;
            }
        }
    }

    Ok(())
}
