use std::collections::HashMap;
use std::collections::HashSet;

use fapt_parse::types::Arch;
use fapt_parse::types::Package;
use fapt_parse::types::PackageType;
use fapt_parse::types::SingleDependency;

use errors::*;

type Id = usize;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct IdKey {
    name: String,
    version: String,
    arches: Vec<Arch>,
}

impl<'a> From<&'a Package> for IdKey {
    fn from(p: &'a Package) -> Self {
        IdKey {
            name: p.name.clone(),
            version: p.version.clone(),
            arches: p.arches.iter().cloned().collect(),
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

type Edge = (Id, Vec<Id>);

#[cfg(never)]
struct Node {
    depends: Vec<Id>,
    recommends: Vec<Id>,
    suggests: Vec<Id>,
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

    pub fn what_kinda(&self) -> () {
        let mut dep: Vec<Edge> = Vec::with_capacity(self.packages.len());
        let mut recommends: Vec<Edge> = Vec::with_capacity(self.packages.len());
        let mut suggests: Vec<Edge> = Vec::with_capacity(self.packages.len());

        for p in &self.packages {
            let bin = match p.style {
                PackageType::Binary(ref bin) => bin,
                PackageType::Source(_) => unreachable!(),
            };
            let id = self.lookup[&p.into()];
            for d in &bin.depends {
                dep.push((id, self.flatten(&d.alternate)));
            }

            for d in &bin.recommends {
                recommends.push((id, self.flatten(&d.alternate)));
            }

            for d in &bin.suggests {
                suggests.push((id, self.flatten(&d.alternate)));
            }
        }
    }

    fn flatten(&self, d: &[SingleDependency]) -> Vec<Id> {
        d.into_iter().flat_map(|d| self.ids(d)).collect()
    }

    // Can't return impl Iterator due to BORROW CHECKER
    fn ids(&self, d: &SingleDependency) -> Vec<Id> {
        self.packages
            .iter()
            .filter(|p| satisfies(p, d))
            .map(|p| self.lookup[&p.into()])
            .collect()
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

fn satisfies(p: &Package, d: &SingleDependency) -> bool {
    if !name_matches(p, d) {
        return false;
    }

    if let Some(arch) = d.arch {
        // TODO: no idea what this logic should be
        if Arch::Any != arch && !p.arches.contains(&arch) {
            return false;
        }
    }

    if !d.arch_filter.is_empty() || !d.stage_filter.is_empty()
    {
        unimplemented!("package\n{:?}\n\nmay satisfy:\n{:?}", p, d)
    }

    for v in &d.version_constraints {
        if !v.satisfied_by(&p.version) {
            return false;
        }
    }

    true
}

fn name_matches(p: &Package, d: &SingleDependency) -> bool {
    if p.name == d.package {
        return true;
    }

    match &p.style {
        PackageType::Binary(bin) => for provides in &bin.provides {
            assert_eq!(1, provides.alternate.len());
            if d.package == provides.alternate[0].package {
                return true;
            }
        },
        PackageType::Source(_) => (),
    }

    false
}
