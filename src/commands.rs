use std::collections::HashMap;
use std::io;

use failure::bail;
use failure::err_msg;
use failure::format_err;
use failure::Error;
use serde_json;

use crate::deps::dep_graph::DepGraph;
use crate::parse::rfc822;
use crate::parse::rfc822::one_line;
use crate::parse::types::Package;
use crate::system::System;

pub fn export(system: &System) -> Result<(), Error> {
    system.walk_sections(|section| {
        serde_json::to_writer(io::stdout(), &section.joined_lines())?;
        println!();
        Ok(())
    })
}

pub fn dodgy_dep_graph(system: &System) -> Result<(), Error> {
    let mut dep_graph = DepGraph::new();

    system.walk_status(|section| {
        // BORROW CHECKER
        let installed_msg = "install ok installed";

        let package = match Package::parse_bin(rfc822::scan(&section)) {
            Ok(package) => package,
            Err(e) => {
                if rfc822::map(&section)?
                    .get("Status")
                    .ok_or_else(|| err_msg("no Status"))?
                    != &vec![installed_msg]
                {
                    return Ok(());
                } else {
                    bail!(e.context(format_err!("parsing:\n{}", section)))
                }
            }
        };

        // TODO: panic?
        if installed_msg != package.unparsed["Status"].join(" ") {
            return Ok(());
        }

        dep_graph.insert(package)?;

        Ok(())
    })?;

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
    system.walk_sections(|map| {
        if map.as_ref().contains_key("Files") {
            print_ninja_source(map.as_ref())
        } else {
            print_ninja_binary(map.as_ref())
        }
    })
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
