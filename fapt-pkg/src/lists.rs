use std::fs;
use std::io;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::vec;

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
pub struct List {
    pub url: Url,
    pub codec: Compression,
    pub compressed_hashes: Hashes,
    pub decompressed_hashes: Hashes,
}

impl List {
    pub fn local_name(&self) -> String {
        hex::encode(self.decompressed_hashes.sha256)
    }
}

pub fn download_files<P: AsRef<Path>>(
    client: &Client,
    lists_dir: P,
    releases: &[Release],
) -> Result<()> {
    let lists = find_files(&releases).chain_err(|| "filtering releases")?;

    let temp_dir =
        TempDir::new_in(&lists_dir, ".fapt-lists").chain_err(|| "creating temporary directory")?;

    let downloads: Vec<fetch::Download> = lists
        .iter()
        .filter_map(|&(_, ref list)| {
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

    fetch::fetch(&client, &downloads).chain_err(|| "downloading listed files")?;

    for (_, list) in lists {
        store_list_item(list, &temp_dir, &lists_dir)?;
    }

    Ok(())
}

fn store_list_item<P: AsRef<Path>, Q: AsRef<Path>>(
    list: List,
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

pub fn find_files(releases: &[Release]) -> Result<Vec<(&Release, List)>> {
    let mut lists = Vec::new();

    for rel in releases {
        let &Release {
            ref req,
            ref file,
            ref sources_entries,
        } = rel;

        let dists = req.dists()?;

        for entry in sources_entries {
            for name in entry.file_names() {
                lists.push((rel, find_file(&dists, &file.contents, &name)?));
            }
        }
    }

    Ok(lists)
}

pub fn walk_all<'i, P: AsRef<Path> + 'i>(
    releases: &'i [Release],
    lists_dir: P,
) -> Result<Box<Iterator<Item = Result<(&'i Release, String)>> + 'i>> {
    Ok(Box::new(find_files(releases)?.into_iter().flat_map(
        move |(release, list)| {
            fs::File::open(lists_dir.as_ref().join(list.local_name()))
                .map(|file| {
                    rfc822::Section::new(file).map(move |maybe_section| {
                        maybe_section.and_then(|block_vec| {
                            String::from_utf8(block_vec)
                                .chain_err(|| format!("section not valid utf-8"))
                                .map(|block| (release, block))
                        })
                    })
                })
                // We have a Result<Iterator<Result<T>>
                // We're in a flatmap, so the return value is Iterator<Result<T>>.
                // I was hoping .unwrap_or_else(|e| [Err(e)].into_iter()) would work.
                // But it doesn't, as the type is not map(closure..).
                // And I couldn't get it it to work with Box, either. That was weirder.
                .expect("couldn't get the error handling to typecheck, sorry")
        },
    )))
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
