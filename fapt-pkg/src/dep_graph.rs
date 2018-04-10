use fapt_parse::deps;
use fapt_parse::deps::Dep;
use std::collections::HashMap;

use errors::*;
use rfc822::mandatory_single_line;

type VersionNumber = String;

// TODO: this is very close to fapt_parse::types

#[derive(Debug)]
struct Deps {
    pre_depends: Vec<Dep>,
    depends: Vec<Dep>,
    recommends: Vec<Dep>,
    suggests: Vec<Dep>,
}

#[derive(Debug)]
struct Version {
    essential: bool,
    deps: Deps,
}

#[derive(Debug)]
struct Package {
    versions: HashMap<VersionNumber, Version>,
}

#[derive(Debug)]
pub struct DepGraph {
    packages: HashMap<String, Package>,
}

impl Package {
    fn new() -> Package {
        Package {
            versions: HashMap::new(),
        }
    }
}

impl DepGraph {
    pub fn new() -> DepGraph {
        DepGraph {
            packages: HashMap::with_capacity(200),
        }
    }

    pub fn read(&mut self, row: &HashMap<&str, Vec<&str>>) -> Result<()> {
        // this is totally going to be reusable or already written
        let name = mandatory_single_line(row, "Package")?;
        let version: VersionNumber = mandatory_single_line(row, "Version")?;
        let deps = Deps {
            pre_depends: parse_dep(row.get("Pre-Depends"))?,
            depends: parse_dep(row.get("Depends"))?,
            recommends: parse_dep(row.get("Recommends"))?,
            suggests: parse_dep(row.get("Suggests"))?,
        };

        // TODO: not technically correct
        let essential = row.contains_key("Essential");

        self.packages
            .entry(name)
            .or_insert_with(Package::new)
            .versions
            .insert(version, Version { deps, essential });
        Ok(())
    }
}

fn parse_dep(multi_str: Option<&Vec<&str>>) -> Result<Vec<Dep>> {
    Ok(match multi_str {
        Some(v) => deps::read(&v.join(" "))?,
        None => Vec::new(),
    })
}
