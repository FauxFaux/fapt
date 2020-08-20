use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;
use chrono::DateTime;
use chrono::Utc;
use filetime;
use reqwest;
use reqwest::header;
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
            .with_context(|| anyhow!("downloading {} to {:?}", download.from, download.to))?;
    }

    Ok(())
}

fn fetch_single(client: &reqwest::Client, download: &Download) -> Result<(), Error> {
    let mut req = client.get(download.from.as_ref());

    if download.to.exists() {
        let when: DateTime<Utc> = ::chrono::DateTime::from(download.to.metadata()?.modified()?);
        req = req.header(header::IF_MODIFIED_SINCE, when.to_rfc2822());
    }

    let mut resp = req.send().with_context(|| anyhow!("initiating request"))?;

    let status = resp.status();
    if reqwest::StatusCode::NOT_MODIFIED == status {
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
        .ok_or_else(|| anyhow!("path must have parent"))?;

    fs::create_dir_all(parent).with_context(|| anyhow!("creating directories: {:?}", parent))?;

    let mut tmp = PersistableTempFile::new_in(parent)
        .with_context(|| anyhow!("couldn't create temporary file"))?;

    if let Some(len) = resp.headers().get(header::CONTENT_LENGTH) {
        tmp.set_len(len.to_str()?.parse()?)
            .with_context(|| anyhow!("pretending to allocate space"))?;
    }

    io::copy(&mut resp, &mut tmp).with_context(|| anyhow!("copying data"))?;

    tmp.persist_by_rename(&download.to)
        .map_err(|e| e.error)
        .with_context(|| anyhow!("persisting result"))?;

    if let Some(modified) = resp.headers().get(header::LAST_MODIFIED) {
        let date = DateTime::parse_from_rfc2822(modified.to_str()?)?;
        let file_time = filetime::FileTime::from_unix_time(date.timestamp(), 0);
        filetime::set_file_times(&download.to, file_time, file_time)?;
    }

    writeln!(io::stderr(), "complete.")?;

    Ok(())
}
