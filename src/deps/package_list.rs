use crate::parse::deps::Dependency;
use crate::parse::deps::SingleDependency;
use crate::Package;

pub struct PackageList {
    pkgs: Vec<Package>,
}

impl PackageList {
    pub fn new() -> PackageList {
        PackageList {
            pkgs: Vec::with_capacity(2 * 1_024),
        }
    }

    pub fn push(&mut self, package: Package) {
        self.pkgs.push(package);
    }

    pub fn find_satisfying(&self, dep: &SingleDependency) -> Vec<&Package> {
        let mut ret = Vec::with_capacity(16);

        for pkg in &self.pkgs {
            if pkg.name == dep.package {
                ret.push(pkg);
            }
        }

        ret
    }
}
