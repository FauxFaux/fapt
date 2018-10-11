use std::collections::hash_map;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use failure::Error;
use failure::ResultExt;
use hex::FromHex;
use reqwest;
use reqwest::Url;

use classic_sources_list::Entry;
use fapt_parse::rfc822;
use fapt_parse::rfc822::mandatory_single_line;
use fapt_parse::rfc822::mandatory_whitespace_list;
use fetch::fetch;
use fetch::Download;
use signing::GpgClient;

use Hashes;

pub struct RequestedReleases {
    releases: Vec<(RequestedRelease, Vec<Entry>)>,
}

#[derive(PartialOrd, Ord, Hash, PartialEq, Eq, Debug)]
pub struct RequestedRelease {
    mirror: Url,
    /// This can also be called "suite" in some places,
    /// e.g. "unstable" (suite) == "sid" (codename)
    codename: String,

    pub arches: Vec<String>,
}

#[derive(Debug)]
pub struct ReleaseFile {
    origin: String,
    label: String,
    suite: Option<String>,
    codename: Option<String>,
    changelogs: Option<String>,
    date: i64,
    valid_until: Option<i64>,
    pub acquire_by_hash: bool,
    pub arches: Vec<String>,
    components: Vec<String>,
    description: Option<String>,
    pub contents: Vec<ReleaseContent>,
}

pub struct ReleaseContent {
    pub len: u64,
    pub name: String,
    pub hashes: Hashes,
}

#[derive(Debug)]
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
            u.port().unwrap_or(0),
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

    pub fn download<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        lists_dir: P,
        keyring_paths: &[Q],
        client: &reqwest::Client,
    ) -> Result<(), Error> {
        let lists_dir = lists_dir.as_ref();

        let mut gpg = GpgClient::new(keyring_paths)?;

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
                Ok(_) => gpg.verify_clearsigned(&dest, &verified),
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
            .with_context(|_| format_err!("verifying {:?} at {:?}", release, dest))?;
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
            .with_context(|_| format_err!("finding release file: {:?}", path.as_ref()))?,
    )
    .read_to_string(&mut file)
    .with_context(|_| format_err!("reading release file: {:?}", path.as_ref()))?;
    Ok(parse_release(&file).with_context(|_| format_err!("parsing {:?}", path.as_ref()))?)
}

fn parse_release(release: &str) -> Result<ReleaseFile, Error> {
    let data = rfc822::map(release)?;
    Ok(ReleaseFile {
        origin: mandatory_single_line(&data, "Origin")?,
        label: mandatory_single_line(&data, "Label")?,
        suite: mandatory_single_line(&data, "Suite").ok(),
        codename: mandatory_single_line(&data, "Codename").ok(),
        changelogs: mandatory_single_line(&data, "Changelogs").ok(),
        date: rfc822::parse_date(&mandatory_single_line(&data, "Date")?)?,
        valid_until: mandatory_single_line(&data, "Valid-Until")
            .and_then(|s| rfc822::parse_date(&s))
            .ok(),
        acquire_by_hash: mandatory_single_line(&data, "Acquire-By-Hash")
            .map(|s| "yes" == s)
            .unwrap_or(false),
        arches: mandatory_whitespace_list(&data, "Architectures")?,
        components: mandatory_whitespace_list(&data, "Components")?,
        description: mandatory_single_line(&data, "Description").ok(),
        contents: load_contents(&data)?,
    })
}

fn load_contents(data: &HashMap<&str, Vec<&str>>) -> Result<Vec<ReleaseContent>, Error> {
    let md5s = take_checksums(data, "MD5Sum")?;
    let sha256s = take_checksums(data, "SHA256")?
        .ok_or_else(|| format_err!("sha256sums missing from release file; refusing to process"))?;

    let mut ret = Vec::with_capacity(sha256s.len());

    for (key, hash) in sha256s {
        let (name, len) = key;

        let mut md5 = [0u8; 16];
        let mut sha256 = [0u8; 32];

        if let Some(md5s) = md5s.as_ref() {
            if let Some(hash) = md5s.get(&key) {
                let v = Vec::from_hex(hash)?;
                ensure!(
                    md5.len() == v.len(),
                    "a md5 checksum isn't the right length? {}",
                    hash
                );
                md5.copy_from_slice(&v);
            }
        }

        {
            let v = Vec::from_hex(hash)?;
            ensure!(
                sha256.len() == v.len(),
                "a sha256 checksum isn't the right length? {}",
                hash
            );

            sha256.copy_from_slice(&v);
        }

        ret.push(ReleaseContent {
            len,
            name: name.to_string(),
            hashes: Hashes { md5, sha256 },
        })
    }

    Ok(ret)
}

fn take_checksums<'a>(
    data: &HashMap<&str, Vec<&'a str>>,
    key: &str,
) -> Result<Option<HashMap<(&'a str, u64), &'a str>>, Error> {
    Ok(match data.get(key) {
        Some(s) => Some(parse_checksums(s)?),
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
