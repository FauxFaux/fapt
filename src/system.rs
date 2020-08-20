//! Lower level operations on a [crate::system::System].
//!
//! ```
//! # fn main() -> Result<(), anyhow::Error> {
//! # use fapt::system::System;
//! let fapt = System::cache_only()?;
//! // ...
//! fapt.update()?;
//! # Ok(())
//! # }
//! ```

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use gpgrv::Keyring;
use reqwest;

use crate::lists;
use crate::parse::Package;
use crate::release;
use crate::rfc822;
use crate::sources_list::Entry;

/// The core object, tying together configuration, caching, and listing.
pub struct System {
    pub(crate) lists_dir: PathBuf,
    dpkg_database: Option<PathBuf>,
    sources_entries: Vec<Entry>,
    arches: Vec<String>,
    keyring: Keyring,
    client: reqwest::blocking::Client,
}

/// A _Listing_ that has been downloaded, and the _Release_ it came from.
#[derive(Debug, Clone)]
pub struct DownloadedList {
    pub release: release::Release,
    pub listing: lists::Listing,
}

impl System {
    /// Produce a `System` with no configuration, using the user's cache directory.
    pub fn cache_only() -> Result<Self, Error> {
        let mut cache_dir = directories::ProjectDirs::from("xxx", "fau", "fapt")
            .ok_or(anyhow!("couldn't find HOME's data directories"))?
            .cache_dir()
            .to_path_buf();
        cache_dir.push("lists");
        Self::cache_only_in(cache_dir)
    }

    /// Produce a `System` with no configuration, using a specified cache directory.
    pub fn cache_only_in<P: AsRef<Path>>(lists_dir: P) -> Result<Self, Error> {
        fs::create_dir_all(lists_dir.as_ref())?;

        let client = if let Ok(proxy) = env::var("http_proxy") {
            reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::http(&proxy)?)
                .build()?
        } else {
            reqwest::blocking::Client::new()
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

    /// Add prepared sources entries.
    ///
    /// These can be acquired from [crate::sources_list]. It is not recommended that you
    /// build them by hand.
    pub fn add_sources_entries<I: IntoIterator<Item = Entry>>(&mut self, entries: I) {
        self.sources_entries.extend(entries);
    }

    /// Configure the architectures this system is using.
    ///
    /// The first architecture is the "primary" architecture.
    pub fn set_arches<S: ToString, I: IntoIterator<Item = S>>(&mut self, arches: I) {
        self.arches = arches.into_iter().map(|x| x.to_string()).collect();
    }

    /// Configure the location of the `dpkg` database.
    ///
    /// This can be used to view `status` information, i.e. information on
    /// currently installed packages.
    pub fn set_dpkg_database<P: AsRef<Path>>(&mut self, dpkg: P) {
        self.dpkg_database = Some(dpkg.as_ref().to_path_buf());
    }

    /// Load GPG keys from an old-style keyring (i.e. not a keybox file).
    ///
    /// Note that this will reject invalid keyring files, unlike other `*apt` implementations.
    pub fn add_keys_from<R: Read>(&mut self, source: R) -> Result<(), Error> {
        self.keyring.append_keys_from(source)?;
        Ok(())
    }

    /// Download any necessary _Listings_ for the configured _Sources Entries_.
    pub fn update(&self) -> Result<(), Error> {
        let requested =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|| anyhow!("parsing sources entries"))?;

        requested
            .download(&self.lists_dir, &self.keyring, &self.client)
            .with_context(|| anyhow!("downloading releases"))?;

        let releases = requested
            .parse(&self.lists_dir)
            .with_context(|| anyhow!("parsing releases"))?;

        lists::download_files(&self.client, &self.lists_dir, &releases)
            .with_context(|| anyhow!("downloading release content"))?;

        Ok(())
    }

    /// Explain the configured _Listings_.
    pub fn listings(&self) -> Result<Vec<DownloadedList>, Error> {
        let releases =
            release::RequestedReleases::from_sources_lists(&self.sources_entries, &self.arches)
                .with_context(|| anyhow!("parsing sources entries"))?
                .parse(&self.lists_dir)
                .with_context(|| anyhow!("parsing releases"))?;

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

    /// Open a `DownloadedList`, to access the packages inside it.
    pub fn open_listing(&self, list: &DownloadedList) -> Result<ListingBlocks, Error> {
        Ok(ListingBlocks {
            inner: lists::sections_in(&list.release, &list.listing, &self.lists_dir)?,
        })
    }

    /// Open the `dpkg` `status` database, to access the packages inside it.
    pub fn open_status(&self) -> Result<ListingBlocks, Error> {
        let mut status = self
            .dpkg_database
            .as_ref()
            .ok_or_else(|| anyhow!("dpkg database not set"))?
            .to_path_buf();
        status.push("status");

        Ok(ListingBlocks {
            inner: rfc822::Blocks::new(fs::File::open(status)?, "status".to_string()),
        })
    }
}

/// The _Blocks_ of a _Listing_.
pub struct ListingBlocks {
    pub(crate) inner: rfc822::Blocks<fs::File>,
}

impl Iterator for ListingBlocks {
    type Item = Result<NamedBlock, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|v| {
            v.map(|inner| NamedBlock {
                inner,
                locality: self.inner.inner.name.to_string(),
            })
        })
    }
}

/// A _Block_ from a _Listing_, with a name (for error reporting).
#[derive(Clone, Debug)]
pub struct NamedBlock {
    locality: String,
    inner: String,
}

impl NamedBlock {
    pub fn as_map(&self) -> Result<rfc822::Map, Error> {
        rfc822::fields_in_block(&self.inner).collect_to_map()
    }

    pub fn as_pkg(&self) -> Result<Package, Error> {
        Package::parse(&mut self.as_map()?)
    }

    pub fn into_string(self) -> String {
        self.inner
    }
}
