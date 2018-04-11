use fapt_parse::deps;
use fapt_parse::deps::Dep;
use std::collections::HashMap;
use std::collections::HashSet;

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
    provides: Vec<Dep>,
}

#[derive(Debug)]
struct Version {
    essential: bool,
    deps: Deps,
}

#[derive(Debug)]
pub struct Package {
    versions: HashMap<VersionNumber, Version>,
}

#[derive(Debug)]
pub struct DepGraph {
    packages: HashMap<String, Package>,
}

#[derive(Debug)]
pub struct Leaves {
    pub aliases: HashMap<String, HashSet<String>>,
    pub direct_dep: HashSet<String>,
    pub maybe_dep: HashSet<String>,
    pub recommended: HashSet<String>,
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
            provides: parse_dep(row.get("Provides"))?,
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

    pub fn sloppy_leaves(&self) -> Leaves {
        let num_packages = self.packages.len();
        let mut direct_dep = HashSet::with_capacity(num_packages / 2);
        let mut maybe_dep = HashSet::with_capacity(num_packages / 10);
        let mut recommended = HashSet::with_capacity(num_packages / 4);
        let mut aliases = HashMap::with_capacity(num_packages / 10);

        for (name, p) in &self.packages {
            for v in p.versions.values() {
                if v.essential {
                    direct_dep.insert(name.to_string());
                }

                for p in &v.deps.provides {
                    assert_eq!(1, p.alternate.len());
                    let p = &p.alternate[0];
                    assert_eq!(None, p.arch);
                    aliases
                        .entry(p.package.to_string())
                        .or_insert_with(HashSet::new)
                        .insert(name.to_string());
                }

                for d in &v.deps.depends {
                    match d.alternate.len() {
                        0 => unreachable!(),
                        1 => {
                            direct_dep.insert(d.alternate[0].package.to_string());
                        }
                        _ => for a in &d.alternate {
                            maybe_dep.insert(a.package.to_string());
                        },
                    }
                }

                for d in &v.deps.recommends {
                    // probably always one, does it matter?
                    for a in &d.alternate {
                        recommended.insert(a.package.to_string());
                    }
                }
            }
        }

        Leaves {
            aliases,
            direct_dep,
            maybe_dep,
            recommended,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Package)> {
        self.packages.iter()
    }
}

fn parse_dep(multi_str: Option<&Vec<&str>>) -> Result<Vec<Dep>> {
    Ok(match multi_str {
        Some(v) => deps::read(&v.join(" "))?,
        None => Vec::new(),
    })
}
