use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use failure::err_msg;
use failure::Error;

use crate::classic_sources_list;
use crate::lists;
use crate::rfc822::RfcMapExt;
use crate::system::DownloadedList;
use crate::system::ListingWalker;
use crate::system::Section;
use crate::system::System;

pub fn add_builtin_keys(system: &mut System) {
    system
        .add_keys_from(io::Cursor::new(distro_keyring::supported_keys()))
        .expect("static data");
}

pub fn add_sources_entries_from_str<S: AsRef<str>>(
    system: &mut System,
    string: S,
) -> Result<(), Error> {
    system.add_sources_entries(classic_sources_list::read(io::Cursor::new(
        string.as_ref(),
    ))?);
    Ok(())
}

pub fn all_paragraphs(system: &System) -> Result<AllParagraphs, Error> {
    let mut listings = system.listings()?;
    let current = system.open_listing(&listings.pop().unwrap())?;
    Ok(AllParagraphs {
        lists_dir: system.lists_dir.to_path_buf(),
        listings,
        current,
    })
}

pub struct AllParagraphs {
    lists_dir: PathBuf,
    listings: Vec<DownloadedList>,
    current: ListingWalker,
}

impl Iterator for AllParagraphs {
    type Item = Result<Section, Error>;

    fn next(&mut self) -> Option<Result<Section, Error>> {
        loop {
            if let Some(paragraph) = self.current.next() {
                return Some(paragraph);
            }

            if let Some(new) = self.listings.pop() {
                let inner = match lists::sections_in(&new.release, &new.listing, &self.lists_dir) {
                    Ok(inner) => inner,
                    Err(e) => return Some(Err(e)),
                };
                self.current = ListingWalker { inner };
                continue;
            }

            return None;
        }
    }
}

pub fn source_ninja(system: &System) -> Result<(), Error> {
    for list in system.listings()? {
        for section in system.open_listing(&list)? {
            let section = section?;
            let map = section.as_map()?;
            if map.contains_key("Files") {
                print_ninja_source(&map)?;
            } else {
                print_ninja_binary(&map)?;
            }
        }
    }
    Ok(())
}

// Sigh, I've already written this.
fn subdir(name: &str) -> &str {
    if name.starts_with("lib") {
        &name[..4]
    } else {
        &name[..1]
    }
}

fn print_ninja_source(map: &HashMap<&str, Vec<&str>>) -> Result<(), Error> {
    let pkg = map.get_value("Package").one_line_req()?;
    let version = map.get_value("Version").one_line_req()?.replace(':', "$:");
    let dir = map.get_value("Directory").one_line_req()?;

    let dsc = map
        .get("Files")
        .ok_or_else(|| err_msg("expecting Files"))?
        .iter()
        .filter(|line| line.ends_with(".dsc"))
        .next()
        .ok_or_else(|| err_msg("expecting a .dsc"))?
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| err_msg("expecting valid dsc block"))?;

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
    let pkg = map.get_value("Package").one_line_req()?;
    let source = map
        .get_value("Source")
        .one_line_req()?
        .split_whitespace()
        .nth(0)
        .unwrap();
    let arch = map.get_value("Architecture").one_line_req()?;
    let version = map.get_value("Version").one_line_req()?.replace(':', "$:");
    let filename = map.get_value("Filename").one_line_req()?;
    let size: u64 = map.get_value("Size").one_line_req()?.parse()?;

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
