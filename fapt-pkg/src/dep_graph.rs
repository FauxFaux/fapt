use std::collections::HashMap;
use std::collections::HashSet;

use deb_version::compare_versions;
use failure::Error;
use fapt_parse::types::Arch;
use fapt_parse::types::Arches;
use fapt_parse::types::ConstraintOperator;
use fapt_parse::types::Package;
use fapt_parse::types::PackageType;
use fapt_parse::types::SingleDependency;

type Id = usize;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct IdKey {
    name: String,
    version: String,
    arches: Vec<Arch>,
}

impl<'a> From<&'a Package> for IdKey {
    fn from(p: &'a Package) -> Self {
        let mut arches: Vec<Arch> = p.arches.iter().cloned().collect();
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

pub type Edge = (Id, Vec<Id>);

pub struct WhatKindaLeaves {
    pub depends: Vec<Edge>,
    pub recommends: Vec<Edge>,
    pub suggests: Vec<Edge>,
}

impl DepGraph {
    pub fn new() -> DepGraph {
        DepGraph {
            packages: Vec::with_capacity(200),
            lookup: HashMap::with_capacity(200),
        }
    }

    pub fn insert(&mut self, package: Package) -> Result<(), Error> {
        let id = self.packages.len();
        self.lookup.insert((&package).into(), id);
        self.packages.push(package);
        Ok(())
    }

    pub fn get(&self, id: Id) -> &Package {
        &self.packages[id]
    }

    pub fn what_kinda(&self) -> WhatKindaLeaves {
        let mut depends: Vec<Edge> = Vec::with_capacity(self.packages.len());
        let mut recommends: Vec<Edge> = Vec::with_capacity(self.packages.len());
        let mut suggests: Vec<Edge> = Vec::with_capacity(self.packages.len());

        for p in &self.packages {
            let bin = match p.style {
                PackageType::Binary(ref bin) => bin,
                PackageType::Source(_) => unreachable!(),
            };
            let id = self.lookup[&p.into()];
            for d in &bin.depends {
                let flat = self.flatten(&d.alternate);
                assert!(
                    !flat.is_empty(),
                    "depends should find something, right? {:?}",
                    d.alternate
                );
                depends.push((id, flat));
            }

            for d in &bin.recommends {
                recommends.push((id, self.flatten(&d.alternate)));
            }

            for d in &bin.suggests {
                suggests.push((id, self.flatten(&d.alternate)));
            }
        }

        WhatKindaLeaves {
            depends,
            recommends,
            suggests,
        }
    }

    pub fn find_named(&self, name: &str) -> Id {
        self.packages
            .iter()
            .enumerate()
            .filter(|&(_, p)| p.name == name)
            .max_by(|&(_, l), &(_, r)| compare_versions(&l.version, &r.version))
            .map(|(id, _)| id)
            .expect(&format!("no such package: {}", name))
    }

    fn flatten(&self, d: &[SingleDependency]) -> Vec<Id> {
        let mut v: Vec<Id> = d.into_iter().flat_map(|d| self.ids(d)).collect();

        // e.g.
        // "Depends: java-runtime | openjdk-8-jdk";
        // "Package openjdk-8-djk; Provides: java-runtime"?
        v.sort_unstable();
        v.dedup_by_key(|v| *v);
        v.shrink_to_fit();

        // Maybe we should use HashSet<> here? I imagine a tiny Vec is way smaller,
        // and can be used as a map key etc.
        v
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

    pub fn iter(&self) -> impl Iterator<Item = usize> {
        (0..self.packages.len())
    }
}

fn satisfies(p: &Package, d: &SingleDependency) -> bool {
    if !satisfies_values(d, &p.name, &p.arches, &p.version) && !provides_satisfies(d, p) {
        return false;
    }

    if !d.arch_filter.is_empty() || !d.stage_filter.is_empty() {
        unimplemented!("package\n{:?}\n\nmay satisfy:\n{:?}", p, d)
    }

    true
}

fn provides_satisfies(d: &SingleDependency, package: &Package) -> bool {
    for p in &package.bin().expect("must be bin").provides {
        assert_eq!(
            1,
            p.alternate.len(),
            "no idea what a Provides with an alternate means"
        );
        let p = &p.alternate[0];
        assert!(
            p.arch.is_none(),
            "no idea what a Provides with an arch means"
        );

        let version = match p.version_constraints.len() {
            0 => &package.version,
            1 => {
                let v = &p.version_constraints[0];
                assert_eq!(
                    ConstraintOperator::Eq,
                    v.operator,
                    "no idea what a provides with non-equal means"
                );
                &v.version
            }
            _ => unimplemented!("no idea what a Provides with multiple version constraints means"),
        };
        if satisfies_values(d, &p.package, &package.arches, version) {
            return true;
        }
    }

    false
}

fn satisfies_values(d: &SingleDependency, name: &str, arches: &Arches, version: &str) -> bool {
    if d.package != name {
        return false;
    }

    if let Some(arch) = d.arch {
        // TODO: no idea what this logic should be
        if Arch::Any != arch && !arches.contains(&arch) {
            return false;
        }
    }

    for v in &d.version_constraints {
        if !v.satisfied_by(version) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use fapt_parse::types::Binary;
    use fapt_parse::types::Constraint;
    use fapt_parse::types::ConstraintOperator;
    use fapt_parse::types::Dependency;
    use fapt_parse::types::Package;
    use fapt_parse::types::PackageType;
    use fapt_parse::types::Priority;
    use fapt_parse::types::SingleDependency;

    #[test]
    fn cant_get_no() {
        let mut p = Package {
            name: "foo".to_string(),
            version: "1.0".to_string(),
            style: PackageType::Binary(Binary {
                provides: vec![Dependency {
                    alternate: vec![SingleDependency {
                        package: "bar".to_string(),
                        version_constraints: vec![Constraint {
                            version: "100".to_string(),
                            operator: ConstraintOperator::Eq,
                        }],
                        ..Default::default()
                    }],
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let mut d = SingleDependency {
            package: "bar".to_string(),
            version_constraints: vec![Constraint {
                version: "90".to_string(),
                operator: ConstraintOperator::Ge,
            }],
            ..Default::default()
        };

        assert!(super::satisfies(&p, &d));
    }
}
