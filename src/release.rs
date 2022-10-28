use std::collections::hash_map;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Error;
use chrono::DateTime;
use chrono::Utc;
use gpgrv::Keyring;
use insideout::InsideOut;
use reqwest;
use reqwest::Url;

use crate::checksum::Hashes;
use crate::fetch::fetch;
use crate::fetch::Download;
use crate::rfc822;
use crate::rfc822::RfcMapExt;
use crate::signing::GpgClient;
use crate::sources_list::Entry;

pub struct RequestedReleases {
    releases: Vec<(RequestedRelease, Vec<Entry>)>,
}

#[derive(Clone, PartialOrd, Ord, Hash, PartialEq, Eq, Debug)]
pub struct RequestedRelease {
    mirror: Url,
    /// This can also be called "suite" in some places,
    /// e.g. "unstable" (suite) == "sid" (codename)
    pub codename: String,

    pub arches: Vec<String>,
    pub untrusted: bool,
}

#[derive(Debug, Clone)]
pub struct ReleaseFile {
    origin: String,
    label: String,
    suite: Option<String>,
    codename: Option<String>,
    changelogs: Option<String>,
    date: DateTime<Utc>,
    valid_until: Option<DateTime<Utc>>,
    pub acquire_by_hash: bool,
    pub arches: Vec<String>,
    components: Vec<String>,
    description: Option<String>,
    pub contents: Vec<ReleaseContent>,
}

#[derive(Clone)]
pub struct ReleaseContent {
    pub len: u64,
    pub name: String,
    pub hashes: Hashes,
}

#[derive(Debug, Clone)]
pub struct Release {
    pub req: RequestedRelease,
    pub sources_entries: Vec<Entry>,
    pub file: ReleaseFile,
}

impl fmt::Debug for ReleaseContent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RC {{ {:?} ({}) {:?} }}",
            self.name, self.len, self.hashes,
        )
    }
}

impl RequestedRelease {
    pub fn dists(&self) -> Result<Url, Error> {
        Ok(self
            .mirror
            .join("dists/")?
            .join(&format!("{}/", self.codename))?)
    }

    pub fn filesystem_safe(&self) -> String {
        let u = &self.mirror;
        let underscore_path = u
            .path_segments()
            .map(|parts| parts.collect::<Vec<&str>>().join("_"))
            .unwrap_or_else(String::new);
        format!(
            "{}_{}_{}_{}_{}_{}",
            u.scheme(),
            u.username(),
            u.host_str().unwrap_or(""),
            u.port_or_known_default().unwrap_or(0),
            underscore_path,
            self.codename
        )
    }

    pub fn download_path<P: AsRef<Path>>(&self, lists_dir: P) -> PathBuf {
        lists_dir
            .as_ref()
            .join(format!("{}_InRelease", self.filesystem_safe()))
    }

    pub fn verified_path<P: AsRef<Path>>(&self, lists_dir: P) -> PathBuf {
        lists_dir
            .as_ref()
            .join(format!("{}_Verified", self.filesystem_safe()))
    }
}

impl RequestedReleases {
    /// A sources list, in entirety, suggests:
    ///  * fetching some "Release" (e.g. `deb.debian.org/debian sid`) files,
    ///  * whitelisting some of its "components" (`main`, `contrib`, `non-free`),
    ///  * and specifying the types of thing to pick from it (`deb`, `deb-src`).
    pub fn from_sources_lists(
        sources_list: &[Entry],
        arches: &[String],
    ) -> Result<RequestedReleases, Error> {
        let mut ret = HashMap::with_capacity(sources_list.len() / 2);

        for entry in sources_list {
            ensure!(
                entry.url.ends_with('/'),
                "urls must end with a '/': {:?}",
                entry.url
            );
            match ret.entry(RequestedRelease {
                mirror: Url::parse(&entry.url)?,
                codename: entry.suite_codename.to_string(),
                arches: arches.to_vec(),
                untrusted: entry.untrusted,
            }) {
                hash_map::Entry::Vacant(vacancy) => {
                    vacancy.insert(vec![entry.clone()]);
                }
                hash_map::Entry::Occupied(mut existing) => existing.get_mut().push(entry.clone()),
            }
        }

        Ok(RequestedReleases {
            releases: ret.into_iter().collect(),
        })
    }

    pub fn download<P: AsRef<Path>>(
        &self,
        lists_dir: P,
        keyring: &Keyring,
        client: &reqwest::blocking::Client,
    ) -> Result<(), Error> {
        let lists_dir = lists_dir.as_ref();

        let mut gpg = GpgClient::new(keyring);

        for &(ref release, _) in &self.releases {
            let dest: PathBuf = release.download_path(lists_dir);
            let verified = release.verified_path(lists_dir);

            match fetch(
                client,
                &[Download::from_to(
                    release.dists()?.join("InRelease")?,
                    &dest,
                )],
            ) {
                Ok(_) => gpg.read_clearsigned(&dest, &verified, !release.untrusted),
                Err(_) => {
                    let mut detatched_signature = dest.as_os_str().to_os_string();
                    detatched_signature.push(".gpg");

                    fetch(
                        client,
                        &[Download::from_to(release.dists()?.join("Release")?, &dest)],
                    )?;

                    fetch(
                        client,
                        &[Download::from_to(
                            release.dists()?.join("Release.gpg")?,
                            &detatched_signature,
                        )],
                    )?;
                    gpg.verify_detached(&dest, detatched_signature, verified)
                }
            }
            .with_context(|| anyhow!("verifying {:?} at {:?}", release, dest))?;
        }

        Ok(())
    }

    pub fn parse<P: AsRef<Path>>(self, lists_dir: P) -> Result<Vec<Release>, Error> {
        self.releases
            .into_iter()
            .map(|(req, sources_entries)| {
                parse_release_file(req.verified_path(&lists_dir)).map(|file| Release {
                    req,
                    file,
                    sources_entries,
                })
            })
            .collect::<Result<Vec<Release>, Error>>()
    }
}

pub fn parse_release_file<P: AsRef<Path>>(path: P) -> Result<ReleaseFile, Error> {
    let mut file = String::with_capacity(100 * 1024);
    io::BufReader::new(
        fs::File::open(path.as_ref())
            .with_context(|| anyhow!("finding release file: {:?}", path.as_ref()))?,
    )
    .read_to_string(&mut file)
    .with_context(|| anyhow!("reading release file: {:?}", path.as_ref()))?;
    Ok(
        parse_release(&file)
            .with_context(|| anyhow!("parsing release file {:?}", path.as_ref()))?,
    )
}

fn parse_release(release: &str) -> Result<ReleaseFile, Error> {
    let mut data = rfc822::fields_in_block(release).collect_to_map()?;
    Ok(ReleaseFile {
        origin: data.remove_value("Origin").one_line_req()?.to_string(),
        label: data.remove_value("Label").one_line_req()?.to_string(),
        suite: data.remove_value("Suite").one_line_owned()?,
        codename: data.remove_value("Codename").one_line_owned()?,
        changelogs: data.remove_value("Changelogs").one_line_owned()?,
        date: rfc822::parse_date(&data.remove_value("Date").one_line_req()?)?,
        valid_until: data
            .remove_value("Valid-Until")
            .one_line()?
            .map(|s| rfc822::parse_date(&s))
            .inside_out()?,
        acquire_by_hash: data
            .remove_value("Acquire-By-Hash")
            .one_line()?
            .map(|s| "yes" == s)
            .unwrap_or(false),
        arches: data.remove_value("Architectures").split_whitespace()?,
        components: data.remove_value("Components").split_whitespace()?,
        description: data.remove_value("Description").one_line_owned()?,
        contents: load_contents(&mut data)?,
    })
}

fn load_contents(data: &mut HashMap<&str, Vec<&str>>) -> Result<Vec<ReleaseContent>, Error> {
    let md5s = take_checksums(data, "MD5Sum")?;
    let sha256s = take_checksums(data, "SHA256")?
        .ok_or_else(|| anyhow!("sha256sums missing from release file; refusing to process"))?;

    let mut ret = Vec::with_capacity(sha256s.len());

    for (key, hash) in sha256s {
        let (name, len) = key;

        let mut md5 = [0u8; 16];

        if let Some(md5s) = md5s.as_ref() {
            if let Some(hash) = md5s.get(&key) {
                md5 = crate::checksum::parse_md5(hash)?;
            }
        }

        let sha256 = crate::checksum::parse_sha256(hash)?;

        ret.push(ReleaseContent {
            len,
            name: name.to_string(),
            hashes: Hashes { md5, sha256 },
        })
    }

    Ok(ret)
}

pub fn take_checksums<'a>(
    data: &mut HashMap<&str, Vec<&'a str>>,
    key: &str,
) -> Result<Option<HashMap<(&'a str, u64), &'a str>>, Error> {
    Ok(match data.remove(key) {
        Some(s) => Some(parse_checksums(&s)?),
        None => None,
    })
}

fn parse_checksums<'s>(lines: &[&'s str]) -> Result<HashMap<(&'s str, u64), &'s str>, Error> {
    let mut ret = HashMap::new();
    for line in lines {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        ensure!(3 == parts.len(), "invalid checksums line: {:?}", line);
        ret.insert((parts[2], parts[1].parse()?), parts[0]);
    }

    Ok(ret)
}
