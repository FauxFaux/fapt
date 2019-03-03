use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use failure::err_msg;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use gpgrv::Keyring;
use reqwest;

use crate::classic_sources_list::Entry;
use crate::lists;
use crate::parse::rfc822;
use crate::release;
use crate::types::Package;

pub struct System {
    pub(crate) lists_dir: PathBuf,
    dpkg_database: Option<PathBuf>,
    sources_entries: Vec<Entry>,
    arches: Vec<String>,
    keyring: Keyring,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct DownloadedList {
    pub release: release::Release,
    pub listing: lists::Listing,
}

impl System {
    pub fn cache_only() -> Result<Self, Error> {
        let mut cache_dir = directories::ProjectDirs::from("xxx", "fau", "fapt")
            .ok_or(err_msg("couldn't find HOME's data directories"))?
            .cache_dir()
            .to_path_buf();
        cache_dir.push("lists");
        Self::cache_only_in(cache_dir)
    }

    pub fn cache_only_in<P: AsRef<Path>>(lists_dir: P) -> Result<Self, Error> {
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
            keyring: Keyring::new(),
            client,
        })
    }

    pub fn add_sources_entries<I: IntoIterator<Item = Entry>>(&mut self, entries: I) {
        self.sources_entries.extend(entries);
    }

    pub fn set_arches<S: ToString, I: IntoIterator<Item = S>>(&mut self, arches: I) {
        self.arches = arches.into_iter().map(|x| x.to_string()).collect();
    }

    pub fn set_dpkg_database<P: AsRef<Path>>(&mut self, dpkg: P) {
        self.dpkg_database = Some(dpkg.as_ref().to_path_buf());
    }

    pub fn add_keys_from<R: Read>(&mut self, source: R) -> Result<(), Error> {
        self.keyring.append_keys_from(source)?;
        Ok(())
    }

    pub fn update(&self) -> Result<(), Error> {
        let requested =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|_| format_err!("parsing sources entries"))?;

        requested
            .download(&self.lists_dir, &self.keyring, &self.client)
            .with_context(|_| format_err!("downloading releases"))?;

        let releases = requested
            .parse(&self.lists_dir)
            .with_context(|_| format_err!("parsing releases"))?;

        lists::download_files(&self.client, &self.lists_dir, &releases)
            .with_context(|_| format_err!("downloading release content"))?;

        Ok(())
    }

    pub fn listings(&self) -> Result<Vec<DownloadedList>, Error> {
        let releases =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|_| format_err!("parsing sources entries"))?
                .parse(&self.lists_dir)
                .with_context(|_| format_err!("parsing releases"))?;

        let mut ret = Vec::with_capacity(releases.len() * 4);

        for release in releases {
            for listing in lists::selected_listings(&release) {
                ret.push(DownloadedList {
                    release: release.clone(),
                    listing,
                });
            }
        }

        Ok(ret)
    }

    pub fn open_listing(&self, list: &DownloadedList) -> Result<ListingWalker, Error> {
        Ok(ListingWalker {
            inner: lists::sections_in(&list.release, &list.listing, &self.lists_dir)?,
        })
    }

    pub fn open_status(&self) -> Result<ListingWalker, Error> {
        let mut status = self
            .dpkg_database
            .as_ref()
            .ok_or_else(|| format_err!("dpkg database not set"))?
            .to_path_buf();
        status.push("status");

        Ok(ListingWalker {
            inner: lists::sections_in_reader(fs::File::open(status)?, "status".to_string()),
        })
    }
}

pub struct ListingWalker {
    pub(crate) inner: rfc822::StringSections<fs::File>,
}

impl Iterator for ListingWalker {
    type Item = Result<Section, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| {
            v.map(|inner| Section {
                inner,
                locality: self.inner.inner.name.to_string(),
            })
        })
    }
}

#[derive(Clone, Debug)]
pub struct Section {
    locality: String,
    inner: String,
}

impl Section {
    pub fn as_map(&self) -> Result<rfc822::Map, Error> {
        rfc822::scan(&self.inner).collect_to_map()
    }

    pub fn as_pkg(&self) -> Result<Package, Error> {
        Package::parse(&mut self.as_map()?)
    }

    pub fn into_string(self) -> String {
        self.inner
    }
}
