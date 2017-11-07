use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use reqwest;
use reqwest::header;
use tempfile_fast::persistable_tempfile_in;

use errors::*;

pub struct Download {
    from: reqwest::Url,
    to: PathBuf,
}

impl Download {
    pub fn from_str_to<P: AsRef<Path>>(from: &str, to: P) -> Result<Self> {
        Ok(Self::from_to(reqwest::Url::parse(from)?, to))
    }

    pub fn from_to<P: AsRef<Path>>(from: reqwest::Url, to: P) -> Self {
        Download {
            from,
            to: to.as_ref().to_path_buf(),
        }
    }
}

pub fn fetch(client: &reqwest::Client, downloads: &[Download]) -> Result<()> {
    // TODO: reqwest parallel API, when it's stable

    for download in downloads {
        println!("Downloading: {}", download.from);
        fetch_single(client, download).chain_err(|| {
            format!("downloading {} to {:?}", download.from, download.to)
        })?;
    }

    Ok(())
}

fn fetch_single(client: &reqwest::Client, download: &Download) -> Result<()> {
    if download.to.exists() {
        // TODO: send If-Modified-Since
        return Ok(());
    }

    let mut resp = client.get(download.from.as_ref()).send().chain_err(
        || "initiating request",
    )?;

    let status = resp.status();
    if !status.is_success() {
        bail!(
            "couldn't download {}: server responded with {:?}",
            download.from,
            status
        );
    }
    let parent = download.to.parent().ok_or("path must have parent")?;

    fs::create_dir_all(parent).chain_err(|| {
        format!("creating directories: {:?}", parent)
    })?;

    let mut tmp = persistable_tempfile_in(parent).chain_err(
        || "couldn't create temporary file",
    )?;

    if let Some(len) = resp.headers().get::<header::ContentLength>() {
        tmp.set_len(**len).chain_err(
            || "pretending to allocate space",
        )?;
    }

    io::copy(&mut resp, tmp.as_mut()).chain_err(
        || "copying data",
    )?;

    tmp.persist_noclobber(&download.to).chain_err(
        || "persisting result",
    )
}
