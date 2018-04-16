use std::collections::HashMap;
use std::collections::HashSet;

use fapt_parse::types::Package;
use fapt_parse::types::PackageType;

use errors::*;

type VersionNumber = String;

#[derive(Debug)]
pub struct NamedPackage {
    versions: HashMap<VersionNumber, Package>,
}

#[derive(Debug)]
pub struct DepGraph {
    packages: HashMap<String, NamedPackage>,
}

#[derive(Debug)]
pub struct Leaves {
    pub aliases: HashMap<String, HashSet<String>>,
    pub direct_dep: HashSet<String>,
    pub maybe_dep: HashSet<String>,
    pub recommended: HashSet<String>,
}

impl NamedPackage {
    fn new() -> NamedPackage {
        NamedPackage {
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

    pub fn read(&mut self, package: Package) -> Result<()> {
        self.packages
            .entry(package.name.to_string())
            .or_insert_with(NamedPackage::new)
            .versions
            .insert(package.version.to_string(), package);
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
                let bin = match v.style {
                    PackageType::Binary(ref bin) => bin,
                    PackageType::Source(_) => unreachable!(),
                };

                if bin.essential {
                    direct_dep.insert(name.to_string());
                }

                for p in &bin.provides {
                    assert_eq!(1, p.alternate.len());
                    let p = &p.alternate[0];
                    assert_eq!(None, p.arch);
                    aliases
                        .entry(p.package.to_string())
                        .or_insert_with(HashSet::new)
                        .insert(name.to_string());
                }

                for d in &bin.depends {
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

                for d in &bin.recommends {
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

    pub fn iter(&self) -> impl Iterator<Item = (&String, &NamedPackage)> {
        self.packages.iter()
    }
}
