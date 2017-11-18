use std::fs;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use flate2::bufread::GzDecoder;
use hex;
use reqwest::Client;
use reqwest::Url;
use tempdir::TempDir;
use tempfile_fast::persistable_tempfile_in;

use checksum;
use fetch;
use release::ReleaseContent;
use release::Release;
use rfc822;
use Hashes;

use errors::*;

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
#[derive(Debug, Hash, PartialEq, Eq)]
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
) -> Result<()> {
    let lists = extract_downloads(releases).chain_err(|| "filtering releases")?;

    let temp_dir =
        TempDir::new_in(&lists_dir, ".fapt-lists").chain_err(|| "creating temporary directory")?;

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

    fetch::fetch(client, &downloads).chain_err(|| "downloading listed files")?;

    for list in lists {
        store_list_item(&list, &temp_dir, &lists_dir)?;
    }

    Ok(())
}

fn store_list_item<P: AsRef<Path>, Q: AsRef<Path>>(
    list: &DownloadableListing,
    temp_dir: P,
    lists_dir: Q,
) -> Result<()> {
    let local_name = list.local_name();
    let destination_path = lists_dir.as_ref().join(&local_name);
    if destination_path.exists() {
        return Ok(());
    }

    let temp_path = temp_dir.as_ref().join(&local_name);
    let mut temp =
        fs::File::open(&temp_path).chain_err(|| "opening a temp file we just downloaded")?;

    checksum::validate(&mut temp, list.compressed_hashes)
        .chain_err(|| format!("validating downloaded file: {:?}", temp_path))?;

    match list.codec {
        Compression::None => fs::rename(temp_path, destination_path)?,
        Compression::Gz => {
            temp.seek(SeekFrom::Start(0))?;
            let mut uncompressed_temp = persistable_tempfile_in(&lists_dir).chain_err(|| {
                format!("making temporary file in {:?}", lists_dir.as_ref())
            })?;

            decompress_gz(temp, uncompressed_temp.as_mut(), list.decompressed_hashes)
                .chain_err(|| format!("decomressing {:?}", temp_path))?;

            uncompressed_temp
                .persist_noclobber(destination_path)
                .chain_err(|| "storing decompressed file")?;
        }
    }

    Ok(())
}

fn decompress_gz<R: Read, F: Read + Write + Seek>(
    mut compressed: R,
    mut uncompressed: F,
    decompressed_hashes: Hashes,
) -> Result<()> {
    io::copy(
        &mut GzDecoder::new(io::BufReader::new(&mut compressed))?,
        &mut uncompressed,
    ).chain_err(|| "decomressing")?;

    uncompressed
        .seek(SeekFrom::Start(0))
        .chain_err(|| "rewinding")?;

    checksum::validate(&mut uncompressed, decompressed_hashes)
        .chain_err(|| "validating decompressed file")?;

    Ok(())
}

pub fn selected_listings(release: &Release) -> Vec<Listing> {
    let mut ret = Vec::new();

    for entry in &release.sources_entries {
        let directory = if entry.src { "source" } else { "binary" };
        let name = if entry.src { "Sources" } else { "Packages" };

        for component in &entry.components {
            ret.push(Listing {
                component: component.to_string(),
                arch: if entry.src {
                    None
                } else {
                    Some("amd64".to_string())
                },
                directory: directory.to_string(),
                name: name.to_string(),
            })
        }
    }

    ret
}

pub fn extract_downloads(releases: &[Release]) -> Result<Vec<DownloadableListing>> {
    releases
        .iter()
        .flat_map(|rel| {
            selected_listings(rel)
                .into_iter()
                .map(|listing| find_file_easy(rel, &listing))
        })
        .collect()
}

pub fn sections_in<'i, P: AsRef<Path> + 'i>(
    release: &'i Release,
    listing: &'i Listing,
    lists_dir: P,
) -> Result<Box<Iterator<Item = Result<String>> + 'i>> {
    Ok(Box::new(
        rfc822::Section::new(open_listing(release, listing, lists_dir)?).map(decode_vec),
    ))
}

fn decode_vec(from: Result<Vec<u8>>) -> Result<String> {
    from.and_then(|vec| String::from_utf8(vec).chain_err(|| "decoding string"))
}

pub fn open_listing<P: AsRef<Path>>(
    release: &Release,
    listing: &Listing,
    lists_dir: P,
) -> Result<fs::File> {
    let local_path = lists_dir
        .as_ref()
        .join(find_file_easy(release, listing)?.local_name());
    fs::File::open(&local_path).chain_err(|| format!("Couldn't open {:?}", local_path))
}

pub fn find_file_easy(release: &Release, listing: &Listing) -> Result<DownloadableListing> {
    find_file(
        &release.req.dists()?,
        &release.file.contents,
        release.file.acquire_by_hash,
        &listing,
    ).chain_err(|| format!("finding {:?} in {:?}", listing, release))
}

pub fn find_file(
    base_url: &Url,
    contents: &[ReleaseContent],
    acquire_by_hash: bool,
    listing: &Listing,
) -> Result<DownloadableListing> {
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

    let raw_hashes = raw_hashes.ok_or_else(|| format!("file {:?} not found in release", base))?;

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
