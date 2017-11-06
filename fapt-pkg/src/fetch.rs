use std::path::Path;
use std::path::PathBuf;
use std::io;

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
        if download.to.exists() {
            // TODO: send If-Modified-Since
            continue;
        }

        let mut resp = client.get(download.from.as_ref()).send()?;
        let status = resp.status();
        if !status.is_success() {
            bail!(
                "couldn't download {}: server responded with {:?}",
                download.from,
                status
            );
        }
        let mut tmp =
            persistable_tempfile_in(download.to.parent().ok_or("path must have parent")?)?;

        if let Some(len) = resp.headers().get::<header::ContentLength>() {
            tmp.set_len(**len)?;
        }

        io::copy(&mut resp, tmp.as_mut())?;

        tmp.persist_noclobber(&download.to)?;
    }

    Ok(())
}
