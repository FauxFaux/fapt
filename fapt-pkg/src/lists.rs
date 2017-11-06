use std::collections::HashSet;
use std::collections::HashMap;

use std::path::Path;
use std::path::PathBuf;

use reqwest;
use reqwest::Url;

use errors::*;
use fetch::fetch;
use fetch::Download;
use rfc822;
use signing::GpgClient;

#[derive(PartialOrd, Ord, Hash, PartialEq, Eq, Debug)]
pub struct RequestedRelease {
    mirror: Url,
    /// This can also be called "suite" in some places,
    /// e.g. "unstable" (suite) == "sid" (codename)
    codename: String,
}

pub struct ReleaseFile {
    origin: String,
    label: String,
    suite: String,
    codename: String,
    changelogs: String,
    date: i64,
    valid_until: i64,
    acquire_by_hash: bool,
    architectures: Vec<String>,
    components: Vec<String>,
    description: String,
    contents: Vec<ReleaseContent>,
}

pub struct ReleaseContent {
    len: u64,
    name: String,
    md5: [u8; 20],
    sha256: [u8; 32],
}

impl RequestedRelease {
    pub fn dists(&self) -> Result<Url> {
        Ok(self.mirror.join("dists/")?.join(
            &format!("{}/", self.codename),
        )?)
    }

    pub fn filesystem_safe(&self) -> String {
        let u = &self.mirror;
        let underscore_path = u.path_segments()
            .map(|parts| parts.collect::<Vec<&str>>().join("_"))
            .unwrap_or_else(|| String::new());
        format!(
            "{}_{}_{}_{}_{}_{}",
            u.scheme(),
            u.username(),
            u.host_str().unwrap_or(""),
            u.port().unwrap_or(0),
            underscore_path,
            self.codename
        )
    }
}

pub fn releases(sources_list: &[::classic_sources_list::Entry]) -> Result<Vec<RequestedRelease>> {
    let mut ret = HashSet::with_capacity(sources_list.len() / 2);

    for entry in sources_list {
        ensure!(entry.url.ends_with('/'), "urls must end with a '/'");
        ret.insert(RequestedRelease {
            mirror: Url::parse(&entry.url)?,
            codename: entry.suite_codename.to_string(),
        });
    }

    Ok(ret.into_iter().collect())
}

pub fn download_releases<P: AsRef<Path>>(
    lists_dir: P,
    releases: &[RequestedRelease],
    keyring_paths: &[&str],
) -> Result<Vec<PathBuf>> {
    let lists_dir = lists_dir.as_ref();

    let mut downloads = Vec::with_capacity(releases.len());

    for release in releases {
        let url = release.dists()?.join("InRelease")?;
        let dest = format!("{}_InRelease", release.filesystem_safe());
        downloads.push(Download::from_to(url, lists_dir.join(dest)));
    }

    let client = reqwest::Client::new();
    fetch(&client, &downloads).chain_err(
        || "downloading releases",
    )?;

    let mut ret = Vec::with_capacity(releases.len());

    let mut gpg = GpgClient::new(keyring_paths)?;

    for release in releases {
        let downloaded = lists_dir.join(format!("{}_InRelease", release.filesystem_safe()));
        let verified = lists_dir.join(format!("{}_Verified", release.filesystem_safe()));
        gpg.verify_clearsigned(downloaded, &verified).chain_err(
            || {
                format!("verifying {:?}", release)
            },
        )?;
        ret.push(verified);
    }
    Ok(ret)
}

fn mandatory_single_line(data: &HashMap<&str, Vec<&str>>, key: &str) -> Result<String> {
    Ok(data.get(key).ok_or_else(|| format!("{} is mandatory", key))?.join(" "))
}

fn parse_release(release: &str) -> Result<ReleaseFile> {
    let data = rfc822::map(release)?;
    Ok(ReleaseFile {
        origin: mandatory_single_line(&data, "Origin")?,
        label: mandatory_single_line(&data, "Label")?,
        suite: mandatory_single_line(&data, "Suite")?,
        codename: mandatory_single_line(&data, "Codename")?,
        changelogs: mandatory_single_line(&data, "Changelogs")?,
        date: rfc822::parse_date(&mandatory_single_line(&data, "Date")?)?,
        valid_until: rfc822::parse_date(&mandatory_single_line(&data, "Valid-Until")?)?,
        acquire_by_hash: true, // TODO
        architectures: Vec::new(), // TODO
        components: Vec::new(), // TODO
        description: String::new(), // TODO
        contents: load_contents(&data)?,
    })
}

fn load_contents(data: &HashMap<&str, Vec<&str>>) -> Result<Vec<ReleaseContent>> {
    unimplemented!()
}
