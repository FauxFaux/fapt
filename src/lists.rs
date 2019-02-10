use std::fs;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use crate::parse::rfc822;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use flate2::bufread::GzDecoder;
use hex;
use reqwest::Client;
use reqwest::Url;
use tempfile_fast::PersistableTempFile;

use crate::checksum;
use crate::checksum::Hashes;
use crate::fetch;
use crate::release::Release;
use crate::release::ReleaseContent;

#[derive(Debug)]
pub enum Compression {
    None,
    Gz,
}

impl Compression {
    fn suffix(&self) -> &'static str {
        use self::Compression::*;
        match *self {
            None => "",
            Gz => ".gz",
        }
    }
}

#[derive(Debug)]
pub struct DownloadableListing {
    pub url: Url,
    pub codec: Compression,
    pub compressed_hashes: Hashes,
    pub decompressed_hashes: Hashes,
}

impl DownloadableListing {
    pub fn local_name(&self) -> String {
        hex::encode(self.decompressed_hashes.sha256)
    }
}

// https://deb.debian.org/debian/dists/unstable/contrib/binary-amd64/Packages.gz
// arch: Some("amd64"),
// component: "contrib",
// directory: "binary",
// name: "packages"
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Listing {
    pub component: String,
    pub arch: Option<String>,
    pub directory: String,
    pub name: String,
}

pub fn download_files<P: AsRef<Path>>(
    client: &Client,
    lists_dir: P,
    releases: &[Release],
) -> Result<(), Error> {
    let lists = extract_downloads(releases).with_context(|_| format_err!("filtering releases"))?;

    let temp_dir = tempfile::Builder::new()
        .prefix(".fapt-lists")
        .tempdir_in(&lists_dir)
        .with_context(|_| format_err!("creating temporary directory"))?;

    let downloads: Vec<fetch::Download> = lists
        .iter()
        .filter_map(|list| {
            let local_name = list.local_name();

            if lists_dir.as_ref().join(&local_name).exists() {
                None
            } else {
                Some(fetch::Download::from_to(
                    list.url.clone(),
                    temp_dir.as_ref().join(local_name),
                ))
            }
        })
        .collect();

    fetch::fetch(client, &downloads).with_context(|_| format_err!("downloading listed files"))?;

    for list in lists {
        store_list_item(&list, &temp_dir, &lists_dir)?;
    }

    Ok(())
}

fn store_list_item<P: AsRef<Path>, Q: AsRef<Path>>(
    list: &DownloadableListing,
    temp_dir: P,
    lists_dir: Q,
) -> Result<(), Error> {
    let local_name = list.local_name();
    let destination_path = lists_dir.as_ref().join(&local_name);
    if destination_path.exists() {
        return Ok(());
    }

    let temp_path = temp_dir.as_ref().join(&local_name);
    let mut temp = fs::File::open(&temp_path)
        .with_context(|_| format_err!("opening a temp file we just downloaded"))?;

    checksum::validate(&mut temp, list.compressed_hashes)
        .with_context(|_| format_err!("validating downloaded file: {:?}", temp_path))?;

    match list.codec {
        Compression::None => fs::rename(temp_path, destination_path)?,
        Compression::Gz => {
            temp.seek(SeekFrom::Start(0))?;
            let mut uncompressed_temp =
                PersistableTempFile::new_in(&lists_dir).with_context(|_| {
                    format_err!("making temporary file in {:?}", lists_dir.as_ref())
                })?;

            decompress_gz(temp, &mut uncompressed_temp, list.decompressed_hashes)
                .with_context(|_| format_err!("decomressing {:?}", temp_path))?;

            uncompressed_temp
                .persist_by_rename(destination_path)
                .map_err(|e| e.error)
                .with_context(|_| format_err!("storing decompressed file"))?;
        }
    }

    Ok(())
}

fn decompress_gz<R: Read, F: Read + Write + Seek>(
    mut compressed: R,
    mut uncompressed: F,
    decompressed_hashes: Hashes,
) -> Result<(), Error> {
    io::copy(
        &mut GzDecoder::new(io::BufReader::new(&mut compressed)),
        &mut uncompressed,
    )
    .with_context(|_| format_err!("decomressing"))?;

    uncompressed
        .seek(SeekFrom::Start(0))
        .with_context(|_| format_err!("rewinding"))?;

    checksum::validate(&mut uncompressed, decompressed_hashes)
        .with_context(|_| format_err!("validating decompressed file"))?;

    Ok(())
}

pub fn selected_listings(release: &Release) -> Vec<Listing> {
    let mut ret = Vec::new();

    for entry in &release.sources_entries {
        let directory = if entry.src { "source" } else { "binary" };
        let name = if entry.src { "Sources" } else { "Packages" };

        for component in &entry.components {
            if entry.src {
                ret.push(Listing {
                    component: component.to_string(),
                    arch: None,
                    directory: directory.to_string(),
                    name: name.to_string(),
                })
            } else {
                for arch in &release.req.arches {
                    if !release.file.arches.contains(arch) {
                        continue;
                    }

                    if let Some(ref entry_arch) = entry.arch {
                        if arch != entry_arch {
                            continue;
                        }
                    }

                    ret.push(Listing {
                        component: component.to_string(),
                        arch: Some(arch.to_string()),
                        directory: directory.to_string(),
                        name: name.to_string(),
                    })
                }
            }
        }
    }

    ret
}

pub fn extract_downloads(releases: &[Release]) -> Result<Vec<DownloadableListing>, Error> {
    releases
        .iter()
        .flat_map(|rel| {
            selected_listings(rel)
                .into_iter()
                .map(move |listing| find_file_easy(rel, &listing))
        })
        .collect()
}

pub fn sections_in<P: AsRef<Path>>(
    release: &Release,
    listing: &Listing,
    lists_dir: P,
) -> Result<rfc822::StringSections<fs::File>, Error> {
    let local_path = lists_dir
        .as_ref()
        .join(find_file_easy(release, listing)?.local_name());
    Ok(sections_in_reader(
        fs::File::open(&local_path)
            .with_context(|_| format_err!("Couldn't open {:?}", local_path))?,
        format!("{:?}", local_path),
    ))
}

pub fn sections_in_reader<R: 'static + Read>(input: R, name: String) -> rfc822::StringSections<R> {
    rfc822::ByteSections::new(input, name).into_string_sections()
}

fn decode_vec(from: Result<Vec<u8>, Error>) -> Result<String, Error> {
    Ok(from
        .and_then(|vec| String::from_utf8(vec).map_err(|e| e.into()))
        .with_context(|_| format_err!("decoding string"))?)
}

pub fn find_file_easy(release: &Release, listing: &Listing) -> Result<DownloadableListing, Error> {
    Ok(find_file(
        &release.req.dists()?,
        &release.file.contents,
        release.file.acquire_by_hash,
        &listing,
    )
    .with_context(|_| format_err!("finding {:?} in {:?}", listing, release))?)
}

pub fn find_file(
    base_url: &Url,
    contents: &[ReleaseContent],
    acquire_by_hash: bool,
    listing: &Listing,
) -> Result<DownloadableListing, Error> {
    let directory = listing
        .arch
        .as_ref()
        .map(|arch| format!("{}-{}", listing.directory, arch))
        .unwrap_or_else(|| listing.directory.to_string());

    let base = format!("{}/{}/{}", listing.component, directory, listing.name);

    let gz_name = format!("{}{}", base, Compression::Gz.suffix());

    let mut gz_hashes = None;
    let mut raw_hashes = None;

    for content in contents {
        if content.name == base {
            raw_hashes = Some(content.hashes);
        } else if content.name == gz_name {
            gz_hashes = Some(content.hashes);
        }
    }

    let raw_hashes =
        raw_hashes.ok_or_else(|| format_err!("file {:?} not found in release", base))?;

    let url = base_url.join(&if acquire_by_hash {
        format!(
            "{}/{}/by-hash/SHA256/{}",
            listing.component,
            directory,
            hex::encode(gz_hashes.unwrap_or(raw_hashes).sha256)
        )
    } else {
        gz_hashes.map(|_| gz_name).unwrap_or(base)
    })?;

    Ok(DownloadableListing {
        url,
        codec: gz_hashes
            .map(|_| Compression::Gz)
            .unwrap_or(Compression::None),
        compressed_hashes: gz_hashes.unwrap_or(raw_hashes),
        decompressed_hashes: raw_hashes,
    })
}
