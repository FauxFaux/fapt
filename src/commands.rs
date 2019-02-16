use std::collections::HashMap;
use std::io;

use failure::bail;
use failure::err_msg;
use failure::format_err;
use failure::Error;

use crate::classic_sources_list;
use crate::deps::dep_graph::DepGraph;
use crate::parse::rfc822::one_line;
use crate::parse::types::Package;
use crate::system::System;
use crate::RfcMapExt;

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

pub fn dodgy_dep_graph(system: &System) -> Result<(), Error> {
    let mut dep_graph = DepGraph::new();

    for section in system.open_status()? {
        let section = section?;

        // BORROW CHECKER
        let installed_msg = "install ok installed";

        let package = match Package::parse(&mut section.as_map()?) {
            Ok(package) => package,
            Err(e) => {
                if section.as_map()?.remove_value("Status").required()? != &[installed_msg] {
                    return Ok(());
                } else {
                    bail!(e.context(format_err!("parsing:\n{}", section.into_string())))
                }
            }
        };

        // TODO: panic?
        if installed_msg != package.unparsed["Status"].join(" ") {
            return Ok(());
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
