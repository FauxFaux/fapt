use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use failure::Error;
use failure::ResultExt;
use fapt_parse;
use fapt_parse::rfc822;
use fapt_parse::rfc822::one_line;
use fapt_parse::rfc822::Line;
use fapt_parse::types::Package;
use reqwest;
use serde_json;

use classic_sources_list::Entry;
use dep_graph::DepGraph;
use lists;
use release;

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
        self.add_sources_entries(::classic_sources_list::read(src)?);
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

    pub fn export(&self) -> Result<(), Error> {
        self.walk_sections(|section| {
            serde_json::to_writer(io::stdout(), &section.joined_lines())?;
            println!();
            Ok(())
        })
    }

    pub fn list_installed(&self) -> Result<(), Error> {
        let mut status = self
            .dpkg_database
            .as_ref()
            .ok_or_else(|| format_err!("dpkg database not set"))?
            .to_path_buf();
        status.push("status");

        let mut dep_graph = DepGraph::new();

        for section in lists::sections_in_reader(fs::File::open(status)?)? {
            // BORROW CHECKER
            let section = section?;
            let installed_msg = "install ok installed";

            let package = match Package::parse_bin(rfc822::scan(&section)) {
                Ok(package) => package,
                Err(e) => {
                    if !status_is(rfc822::scan(&section), installed_msg)? {
                        continue;
                    } else {
                        bail!(e.context(format_err!("parsing:\n{}", section)))
                    }
                }
            };

            // TODO: panic?
            if installed_msg != package.unparsed["Status"].join(" ") {
                continue;
            }

            dep_graph.insert(package)?;
        }

        let mut unexplained: Vec<usize> = Vec::with_capacity(100);
        let mut depended: Vec<usize> = Vec::with_capacity(100);
        let mut alt_depended: Vec<usize> = Vec::with_capacity(100);
        let mut only_recommended: Vec<usize> = Vec::with_capacity(100);

        let mut leaves = dep_graph.what_kinda();

        leaves.depends.extend(vec![
            (0, vec![dep_graph.find_named("ubuntu-minimal")]),
            (0, vec![dep_graph.find_named("ubuntu-standard")]),
        ]);

        'packages: for p in dep_graph.iter() {
            for (_src, dest) in &leaves.depends {
                assert!(!dest.is_empty());
                if dest.contains(&p) {
                    match dest.len() {
                        0 => unreachable!(),
                        1 => {
                            depended.push(p);
                            continue 'packages;
                        }
                        _ => {
                            alt_depended.push(p);
                            continue 'packages;
                        }
                    }
                }
            }

            for (_src, dest) in &leaves.recommends {
                if dest.contains(&p) {
                    only_recommended.push(p);
                    continue 'packages;
                }
            }

            unexplained.push(p);
        }

        if false {
            println!("Packages are clearly required");
            println!("=============================");
            println!();

            for p in stringify_package_list(&dep_graph, depended) {
                println!("{}", p);
            }

            println!();
            println!();
            println!("Packages may sometimes be required");
            println!("==================================");
            println!();

            for p in stringify_package_list(&dep_graph, alt_depended) {
                println!("{}", p);
            }

            println!();
            println!();
            println!("Packages are recommended by something");
            println!("=====================================");
            println!();

            for p in stringify_package_list(&dep_graph, only_recommended) {
                println!("{}", p);
            }

            println!();
            println!();
            println!("Unexplained packages");
            println!("====================");
            println!();
        }

        for p in stringify_package_list(&dep_graph, unexplained) {
            println!("{}", p);
        }

        Ok(())
    }

    pub fn source_ninja(&self) -> Result<(), Error> {
        self.walk_sections(|map| {
            if map.as_ref().contains_key("Files") {
                print_ninja_source(map.as_ref())
            } else {
                print_ninja_binary(map.as_ref())
            }
        })
    }
}

fn status_is<'l, I: Iterator<Item = Result<Line<'l>, Error>>>(
    it: I,
    what: &str,
) -> Result<bool, Error> {
    for line in it {
        let (k, v) = line?;
        if "Status" != k {
            continue;
        }

        return Ok(what == one_line(&v)?);
    }

    Ok(false)
}

fn stringify_package_list<I: IntoIterator<Item = usize>>(
    dep_graph: &DepGraph,
    it: I,
) -> impl Iterator<Item = String> {
    let mut vec: Vec<String> = it
        .into_iter()
        .map(|id| format!("{}", dep_graph.get(id).name))
        .collect();
    vec.sort_unstable();
    vec.into_iter()
}

// Sigh, I've already written this.
fn subdir(name: &str) -> &str {
    if name.starts_with("lib") {
        &name[..4]
    } else {
        &name[..1]
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

#[cfg(never)]
struct Sections<'i> {
    lists_dir: PathBuf,
    releases: Box<Iterator<Item = release::Release> + 'i>,
    release: release::Release,
    listings: Box<Iterator<Item = lists::Listing> + 'i>,
    sections: Box<Iterator<Item = Result<String>>>,
}

#[cfg(never)]
impl<'i> Iterator for Sections<'i> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(section) = self.sections.next() {
                return Some(section);
            }

            if let Some(listing) = self.listings.next() {
                // peek() also doesn't live long enough
                self.sections = match lists::sections_in(&self.release, &listing, self.lists_dir) {
                    Ok(sections) => sections,
                    Err(e) => return Some(Err(e)),
                };
                continue;
            }
        }
    }
}

fn print_ninja_source(map: &HashMap<&str, Vec<&str>>) -> Result<(), Error> {
    let pkg = one_line(&map["Package"])?;
    let version = one_line(&map["Version"])?.replace(':', "$:");
    let dir = one_line(&map["Directory"])?;

    let dsc = map["Files"]
        .iter()
        .filter(|line| line.ends_with(".dsc"))
        .next()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap();

    let size: u64 = map["Files"]
        .iter()
        .map(|line| {
            let num: &str = line.split_whitespace().nth(1).unwrap();
            let num: u64 = num.parse().unwrap();
            num
        })
        .sum();

    let prefix = format!("{}/{}_{}", subdir(pkg), pkg, version);

    println!("build $dest/{}$suffix: process-source | $script", prefix);

    println!("  description = PS {} {}", pkg, version);
    println!("  pkg = {}", pkg);
    println!("  version = {}", version);
    println!("  url = $mirror/{}/{}", dir, dsc);
    println!("  prefix = {}", prefix);
    println!("  size = {}", size);
    if size > 250 * 1024 * 1024 {
        // ~20 packages
        println!("  pool = massive")
    } else if size > 100 * 1024 * 1024 {
        // <1%
        println!("  pool = big")
    }

    Ok(())
}

fn print_ninja_binary(map: &HashMap<&str, Vec<&str>>) -> Result<(), Error> {
    let pkg = one_line(&map["Package"])?;
    let source = one_line(&map.get("Source").unwrap_or_else(|| &map["Package"]))?
        .split_whitespace()
        .nth(0)
        .unwrap();
    let arch = one_line(&map["Architecture"])?;
    let version = one_line(&map["Version"])?.replace(':', "$:");
    let filename = one_line(&map["Filename"])?;
    let size: u64 = one_line(&map["Size"])?.parse()?;

    let prefix = format!("{}/{}/{}_{}_{}", subdir(source), source, pkg, version, arch);

    println!("build $dest/{}$suffix: process-binary | $script", prefix);
    println!("  description = PB {} {} {} {}", source, pkg, version, arch);
    println!("  source = {}", source);
    println!("  pkg = {}", pkg);
    println!("  version = {}", version);
    println!("  arch = {}", arch);
    println!("  url = $mirror/{}", filename);
    println!("  prefix = {}", prefix);

    if size > 250 * 1024 * 1024 {
        println!("  pool = massive")
    } else if size > 100 * 1024 * 1024 {
        println!("  pool = big")
    }

    Ok(())
}
