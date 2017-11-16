use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use reqwest;
use serde_json;

use classic_sources_list::Entry;
use release;
use rfc822;
use lists;

use errors::*;

pub struct System {
    lists_dir: PathBuf,
    sources_entries: Vec<Entry>,
    keyring_paths: Vec<PathBuf>,
    client: reqwest::Client,
}

impl System {
    pub fn cache_dirs_only<P: AsRef<Path>>(lists_dir: P) -> Result<Self> {
        fs::create_dir_all(lists_dir.as_ref())?;

        Ok(System {
            lists_dir: lists_dir.as_ref().to_path_buf(),
            sources_entries: Vec::new(),
            keyring_paths: Vec::new(),
            client: reqwest::Client::new(),
        })
    }

    pub fn add_sources_entries<I: Iterator<Item = Entry>>(&mut self, entries: I) {
        self.sources_entries.extend(entries);
    }

    pub fn add_keyring_paths<P: AsRef<Path>, I: Iterator<Item = P>>(
        &mut self,
        keyrings: I,
    ) -> Result<()> {
        self.keyring_paths
            .extend(keyrings.map(|x| x.as_ref().to_path_buf()));
        Ok(())
    }

    pub fn update(&self) -> Result<()> {
        let requested = release::RequestedReleases::from_sources_lists(&self.sources_entries)
            .chain_err(|| "parsing sources entries")?;

        requested
            .download(&self.lists_dir, &self.keyring_paths)
            .chain_err(|| "downloading releases")?;

        let releases = requested
            .parse(&self.lists_dir)
            .chain_err(|| "parsing releases")?;

        lists::download_files(&self.client, &self.lists_dir, &releases)
            .chain_err(|| "downloading release content")?;

        Ok(())
    }

    pub fn export(&self) -> Result<()> {
        let releases = release::RequestedReleases::from_sources_lists(&self.sources_entries)
            .chain_err(|| "parsing sources entries")?
            .parse(&self.lists_dir)
            .chain_err(|| "parsing releases")?;

        for result in lists::walk_all(&releases, &self.lists_dir)? {
            let (release, sections) = result?;
            for section in sections {
                let section = String::from_utf8(section?)?;
                let map = rfc822::map(&section).chain_err(|| format!("scanning {:?}", release))?;
                serde_json::to_writer(io::stdout(), &map)?;
                println!();
            }
        }

        Ok(())
    }
}
