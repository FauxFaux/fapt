use std::collections::HashMap;
use std::env;
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
    arches: Vec<String>,
    keyring_paths: Vec<PathBuf>,
    client: reqwest::Client,
}

impl System {
    pub fn cache_dirs_only<P: AsRef<Path>>(lists_dir: P) -> Result<Self> {
        fs::create_dir_all(lists_dir.as_ref())?;

        let client = if let Ok(proxy) = env::var("http_proxy") {
            reqwest::Client::builder()
                .proxy(reqwest::Proxy::http(&proxy)?)
                .build()?
        } else {
            reqwest::Client::new()
        };

        Ok(System {
            lists_dir: lists_dir.as_ref().to_path_buf(),
            sources_entries: Vec::new(),
            arches: Vec::new(),
            keyring_paths: Vec::new(),
            client,
        })
    }

    pub fn add_sources_entries<I: Iterator<Item = Entry>>(&mut self, entries: I) {
        self.sources_entries.extend(entries);
    }

    pub fn set_arches(&mut self, arches: &[&str]) {
        self.arches = arches.iter().map(|x| x.to_string()).collect();
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
        let requested =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .chain_err(|| "parsing sources entries")?;

        requested
            .download(&self.lists_dir, &self.keyring_paths, &self.client)
            .chain_err(|| "downloading releases")?;

        let releases = requested
            .parse(&self.lists_dir)
            .chain_err(|| "parsing releases")?;

        lists::download_files(&self.client, &self.lists_dir, &releases)
            .chain_err(|| "downloading release content")?;

        Ok(())
    }

    pub fn export(&self) -> Result<()> {
        let releases =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .chain_err(|| "parsing sources entries")?
                .parse(&self.lists_dir)
                .chain_err(|| "parsing releases")?;

        for release in releases {
            for listing in lists::selected_listings(&release) {
                for section in lists::sections_in(&release, &listing, &self.lists_dir)? {
                    let section = section?;
                    let map: HashMap<&str, String> = rfc822::map(&section)
                        .chain_err(|| format!("scanning {:?}", release))?
                        .into_iter()
                        .map(|(k, v)| (k, v.join("\n")))
                        .collect();
                    serde_json::to_writer(io::stdout(), &map)?;
                    println!();
                }
            }
        }

        Ok(())
    }
}
