use std::fs;
use std::io;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;

use flate2::bufread::GzDecoder;
use reqwest::Client;
use reqwest::Url;
use tempdir::TempDir;
use tempfile_fast::persistable_tempfile_in;

use checksum;
use fetch;
use release::ReleaseContent;
use release::Release;
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
pub struct List {
    pub url: Url,
    pub codec: Compression,
    pub compressed_hashes: Hashes,
    pub decompressed_hashes: Hashes,
}

impl List {
    pub fn local_name(&self) -> String {
        use hex::ToHex;
        self.decompressed_hashes.sha256.to_hex()
    }
}

pub fn download_files<P: AsRef<Path>>(client: &Client, lists_dir: P, releases: &[Release]) -> Result<()> {
    let lists = find_files(&releases).chain_err(
        || "filtering releases",
    )?;

    let temp_dir = TempDir::new_in(&lists_dir, ".fapt-lists").chain_err(
        || "creating temporary directory",
    )?;

    let downloads: Vec<fetch::Download> = lists
        .iter()
        .filter_map(|list| {
            let local_name = list.local_name();

            match lists_dir.as_ref().join(&local_name).exists() {
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
        let destination_path = lists_dir.as_ref().join(&local_name);
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
            Compression::None => fs::rename(temp_path, destination_path)?,
            Compression::Gz => {
                let mut uncompressed_temp = persistable_tempfile_in(&lists_dir)?;
                temp.seek(SeekFrom::Start(0))?;

                io::copy(
                    &mut GzDecoder::new(io::BufReader::new(&mut temp))?,
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

pub fn find_files(releases: &[Release]) -> Result<Vec<List>> {
    let mut lists = Vec::new();

    for &Release {
        ref req,
        ref file,
        ref sources_entries,
    } in releases
    {
        let dists = req.dists()?;

        for entry in sources_entries {
            for name in entry.file_names() {
                lists.push(find_file(&dists, &file.contents, &name)?);
            }
        }
    }

    Ok(lists)
}

pub fn find_file(base_url: &Url, contents: &[ReleaseContent], base: &str) -> Result<List> {

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

    let raw_hashes = raw_hashes.ok_or("file not found in release")?;

    Ok(match gz_hashes {
        Some(gz_hashes) => List {
            url: base_url.join(&gz_name)?,
            codec: Compression::Gz,
            compressed_hashes: gz_hashes,
            decompressed_hashes: raw_hashes,
        },
        None => List {
            url: base_url.join(base)?,
            codec: Compression::None,
            compressed_hashes: raw_hashes,
            decompressed_hashes: raw_hashes,
        },
    })
}
