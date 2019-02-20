use crate::Package;

pub struct PackageList {
    deps: Vec<Package>,
}

impl PackageList {
    pub fn new() -> PackageList {
        PackageList {
            deps: Vec::with_capacity(2 * 1_024),
        }
    }
    pub fn push(&mut self, package: Package) {
        self.deps.push(package);
    }
}
