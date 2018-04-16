use std::collections::HashMap;
use std::collections::HashSet;

use fapt_parse::types::Package;
use fapt_parse::types::PackageType;

use errors::*;

type Id = usize;

#[derive(Clone, Debug, Ord, PartialOrd, PartialEq, Eq, Hash)]
struct IdKey {
    name: String,
    version: String,
    arches: Vec<String>,
}

impl<'a> From<&'a Package> for IdKey {
    fn from(p: &'a Package) -> Self {
        let mut arches = p.arch.clone();
        arches.sort_unstable();
        IdKey {
            name: p.name.clone(),
            version: p.version.clone(),
            arches,
        }
    }
}

#[derive(Debug)]
pub struct DepGraph {
    packages: Vec<Package>,
    lookup: HashMap<IdKey, Id>,
}

#[derive(Debug)]
pub struct Leaves {
    pub aliases: HashMap<String, HashSet<String>>,
    pub direct_dep: HashSet<String>,
    pub maybe_dep: HashSet<String>,
    pub recommended: HashSet<String>,
}

impl DepGraph {
    pub fn new() -> DepGraph {
        DepGraph {
            packages: Vec::with_capacity(200),
            lookup: HashMap::with_capacity(200),
        }
    }

    pub fn insert(&mut self, package: Package) -> Result<()> {
        let id = self.packages.len();
        self.lookup.insert((&package).into(), id);
        self.packages.push(package);
        Ok(())
    }

    pub fn sloppy_leaves(&self) -> Leaves {
        let num_packages = self.packages.len();
        let mut direct_dep = HashSet::with_capacity(num_packages / 2);
        let mut maybe_dep = HashSet::with_capacity(num_packages / 10);
        let mut recommended = HashSet::with_capacity(num_packages / 4);
        let mut aliases = HashMap::with_capacity(num_packages / 10);

        for v in &self.packages {
            let name = &v.name;
            {
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

    pub fn iter(&self) -> impl Iterator<Item = &Package> {
        self.packages.iter()
    }
}
