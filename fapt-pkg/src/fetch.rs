use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use failure::Error;
use failure::ResultExt;
use filetime;
use reqwest;
use reqwest::header;
use reqwest::header::IfModifiedSince;
use tempfile_fast::PersistableTempFile;

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

pub fn fetch(client: &reqwest::Client, downloads: &[Download]) -> Result<(), Error> {
    // TODO: reqwest parallel API, when it's stable

    for download in downloads {
        write!(io::stderr(), "Downloading: {} ... ", download.from)?;
        io::stderr().flush()?;
        fetch_single(client, download)
            .with_context(|_| format_err!("downloading {} to {:?}", download.from, download.to))?;
    }

    Ok(())
}

fn fetch_single(client: &reqwest::Client, download: &Download) -> Result<(), Error> {
    let mut req = client.get(download.from.as_ref());

    if download.to.exists() {
        req.header(IfModifiedSince(download.to.metadata()?.modified()?.into()));
    }

    let mut resp = req
        .send()
        .with_context(|_| format_err!("initiating request"))?;

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
    let parent = download
        .to
        .parent()
        .ok_or_else(|| format_err!("path must have parent"))?;

    fs::create_dir_all(parent).with_context(|_| format_err!("creating directories: {:?}", parent))?;

    let mut tmp = PersistableTempFile::new_in(parent)
        .with_context(|_| format_err!("couldn't create temporary file"))?;

    if let Some(len) = resp.headers().get::<header::ContentLength>() {
        tmp.set_len(**len)
            .with_context(|_| format_err!("pretending to allocate space"))?;
    }

    io::copy(&mut resp, &mut tmp).with_context(|_| format_err!("copying data"))?;

    tmp.persist_by_rename(&download.to)
        .map_err(|e| e.error)
        .with_context(|_| format_err!("persisting result"))?;

    if let Some(modified) = resp.headers().get::<header::LastModified>() {
        let file_time = filetime::FileTime::from_system_time(SystemTime::from(**modified));
        filetime::set_file_times(&download.to, file_time, file_time)?;
    }

    writeln!(io::stderr(), "complete.")?;

    Ok(())
}
