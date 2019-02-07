use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use failure::format_err;
use failure::Error;
use failure::ResultExt;
use reqwest;

use crate::classic_sources_list::Entry;
use crate::lists;
use crate::parse::rfc822;
use crate::release;

pub struct System {
    lists_dir: PathBuf,
    dpkg_database: Option<PathBuf>,
    sources_entries: Vec<Entry>,
    arches: Vec<String>,
    keyring_paths: Vec<PathBuf>,
    client: reqwest::Client,
}

impl System {
    pub fn cache_dirs_only<P: AsRef<Path>>(lists_dir: P) -> Result<Self, Error> {
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
            dpkg_database: None,
            sources_entries: Vec::new(),
            arches: Vec::new(),
            keyring_paths: Vec::new(),
            client,
        })
    }

    pub fn add_sources_entry_line(&mut self, src: &str) -> Result<(), Error> {
        self.add_sources_entries(crate::classic_sources_list::read(src)?);
        Ok(())
    }

    pub fn add_sources_entries<I: IntoIterator<Item = Entry>>(&mut self, entries: I) {
        self.sources_entries.extend(entries);
    }

    pub fn set_arches(&mut self, arches: &[&str]) {
        self.arches = arches.iter().map(|x| x.to_string()).collect();
    }

    pub fn set_dpkg_database<P: AsRef<Path>>(&mut self, dpkg: P) {
        self.dpkg_database = Some(dpkg.as_ref().to_path_buf());
    }

    pub fn add_keyring_paths<P: AsRef<Path>, I: IntoIterator<Item = P>>(
        &mut self,
        keyrings: I,
    ) -> Result<(), Error> {
        self.keyring_paths
            .extend(keyrings.into_iter().map(|x| x.as_ref().to_path_buf()));
        Ok(())
    }

    pub fn update(&self) -> Result<(), Error> {
        let requested =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|_| format_err!("parsing sources entries"))?;

        requested
            .download(&self.lists_dir, &self.keyring_paths, &self.client)
            .with_context(|_| format_err!("downloading releases"))?;

        let releases = requested
            .parse(&self.lists_dir)
            .with_context(|_| format_err!("parsing releases"))?;

        lists::download_files(&self.client, &self.lists_dir, &releases)
            .with_context(|_| format_err!("downloading release content"))?;

        Ok(())
    }

    pub fn walk_sections<F>(&self, mut walker: F) -> Result<(), Error>
    where
        F: FnMut(StringSection) -> Result<(), Error>,
    {
        let releases =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|_| format_err!("parsing sources entries"))?
                .parse(&self.lists_dir)
                .with_context(|_| format_err!("parsing releases"))?;

        for release in releases {
            for listing in lists::selected_listings(&release) {
                for section in lists::sections_in(&release, &listing, &self.lists_dir)? {
                    let section = section?;
                    walker(StringSection {
                        inner: rfc822::map(&section)
                            .with_context(|_| format_err!("loading section: {:?}", section))?,
                    })
                    .with_context(|_| format_err!("processing section"))?;
                }
            }
        }
        Ok(())
    }

    pub fn walk_status<F>(&self, mut walker: F) -> Result<(), Error>
    where
        F: FnMut(String) -> Result<(), Error>,
    {
        let mut status = self
            .dpkg_database
            .as_ref()
            .ok_or_else(|| format_err!("dpkg database not set"))?
            .to_path_buf();
        status.push("status");

        for section in lists::sections_in_reader(fs::File::open(status)?)? {
            let section = section?;
            walker(section)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringSection<'s> {
    inner: HashMap<&'s str, Vec<&'s str>>,
}

impl<'s> StringSection<'s> {
    pub fn joined_lines(&self) -> HashMap<&str, String> {
        self.inner.iter().map(|(&k, v)| (k, v.join("\n"))).collect()
    }

    pub fn get_if_one_line(&self, key: &str) -> Option<&str> {
        match self.inner.get(key) {
            Some(list) => match list.len() {
                1 => Some(list[0]),
                _ => None,
            },
            None => None,
        }
    }
}

impl<'s> AsRef<HashMap<&'s str, Vec<&'s str>>> for StringSection<'s> {
    fn as_ref(&self) -> &HashMap<&'s str, Vec<&'s str>> {
        &self.inner
    }
}
