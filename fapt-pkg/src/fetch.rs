use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use filetime;

use reqwest;
use reqwest::header;
use reqwest::header::IfModifiedSince;

use tempfile_fast::persistable_tempfile_in;

use errors::*;

pub struct Download {
    from: reqwest::Url,
    to: PathBuf,
}

impl Download {
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
        write!(io::stderr(), "Downloading: {} ... ", download.from)?;
        io::stderr().flush()?;
        fetch_single(client, download).chain_err(|| {
            format!("downloading {} to {:?}", download.from, download.to)
        })?;
    }

    Ok(())
}

fn fetch_single(client: &reqwest::Client, download: &Download) -> Result<()> {
    let mut req = client.get(download.from.as_ref());

    if download.to.exists() {
        req.header(IfModifiedSince(download.to.metadata()?.modified()?.into()));
    }

    let mut resp = req.send().chain_err(|| "initiating request")?;

    let status = resp.status();
    if reqwest::StatusCode::NotModified == status {
        writeln!(io::stderr(), "already up to date.")?;
        return Ok(());
    } else if !status.is_success() {
        bail!(
            "couldn't download {}: server responded with {:?}",
            download.from,
            status
        );
    }
    let parent = download.to.parent().ok_or("path must have parent")?;

    fs::create_dir_all(parent).chain_err(|| format!("creating directories: {:?}", parent))?;

    let mut tmp = persistable_tempfile_in(parent).chain_err(|| "couldn't create temporary file")?;

    if let Some(len) = resp.headers().get::<header::ContentLength>() {
        tmp.set_len(**len)
            .chain_err(|| "pretending to allocate space")?;
    }

    io::copy(&mut resp, tmp.as_mut()).chain_err(|| "copying data")?;

    if download.to.exists() {
        fs::remove_file(&download.to).chain_err(|| "removing destination for overwriting")?;
    }

    tmp.persist_noclobber(&download.to)
        .chain_err(|| "persisting result")?;

    if let Some(modified) = resp.headers().get::<header::LastModified>() {
        // YAY fourteen date apis
        let since_epoch = SystemTime::from(**modified).duration_since(UNIX_EPOCH)?;
        let file_time = filetime::FileTime::from_seconds_since_1970(
            since_epoch.as_secs(),
            since_epoch.subsec_nanos(),
        );
        filetime::set_file_times(&download.to, file_time, file_time)?;
    }

    writeln!(io::stderr(), "complete.")?;

    Ok(())
}
