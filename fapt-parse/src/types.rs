use std::cmp;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::FromStr;

use deb_version::compare_versions;

use deps;
use errors::*;
use rfc822;

/// The parsed top-level types for package
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageType {
    Source(Source),
    Binary(Binary),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub priority: Priority,
    pub arches: Arches,

    pub maintainer: Vec<Identity>,
    pub original_maintainer: Vec<Identity>,

    pub unparsed: HashMap<String, Vec<String>>,

    pub style: PackageType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub format: SourceFormat,

    pub binaries: Vec<SourceBinary>,
    pub files: Vec<File>,
    pub vcs: Vec<Vcs>,

    pub build_dep: Vec<Dependency>,
    pub build_dep_arch: Vec<Dependency>,
    pub build_dep_indep: Vec<Dependency>,
    pub build_conflict: Vec<Dependency>,
    pub build_conflict_arch: Vec<Dependency>,
    pub build_conflict_indep: Vec<Dependency>,

    pub uploaders: Vec<Identity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binary {
    // "File" is missing in e.g. dpkg/status, but never in Packages as far as I've seen
    pub file: Option<File>,

    pub essential: bool,
    pub build_essential: bool,

    pub installed_size: u64,

    pub description: String,

    pub depends: Vec<Dependency>,
    pub recommends: Vec<Dependency>,
    pub suggests: Vec<Dependency>,
    pub enhances: Vec<Dependency>,
    pub pre_depends: Vec<Dependency>,

    pub breaks: Vec<Dependency>,
    pub conflicts: Vec<Dependency>,
    pub replaces: Vec<Dependency>,

    pub provides: Vec<Dependency>,
}

// The dependency chain types

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub alternate: Vec<SingleDependency>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleDependency {
    pub package: String,
    pub arch: Option<Arch>,
    /// Note: It's possible Debian only supports a single version constraint.
    pub version_constraints: Vec<Constraint>,
    pub arch_filter: Arches,
    pub stage_filter: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub version: String,
    pub operator: ConstraintOperator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintOperator {
    Ge,
    Eq,
    Le,
    Gt,
    Lt,
}

// Other types

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Arch {
    Any,
    All,
    Amd64,
    Armel,
    Armhf,
    Arm64,
    I386,
    Mips,
    Mipsel,
    Mips64,
    Mips64El,
    Ppc64El,
    S390X,
}

bitflags! {
    /// Having any/all as part of this is a bit of a cop-out, I think.
    /// I'm pretty sure it's okay to list "any amd64" as an arch. I should find a spec.
    pub struct Arches: u16 {
        const ANY      = 1 << Arch::Any      as usize;
        const ALL      = 1 << Arch::All      as usize;
        const AMD64    = 1 << Arch::Amd64    as usize;
        const ARMEL    = 1 << Arch::Armel    as usize;
        const ARMHF    = 1 << Arch::Armhf    as usize;
        const ARM64    = 1 << Arch::Arm64    as usize;
        const I386     = 1 << Arch::I386     as usize;
        const MIPS     = 1 << Arch::Mips     as usize;
        const MIPSEL   = 1 << Arch::Mipsel   as usize;
        const MIPS64   = 1 << Arch::Mips64   as usize;
        const MIPS64EL = 1 << Arch::Mips64El as usize;
        const PPC64EL  = 1 << Arch::Ppc64El  as usize;
        const S390X    = 1 << Arch::S390X    as usize;
    }
}

impl Arch {
    fn as_flag(&self) -> Arches {
        match *self {
            Arch::Any => Arches::ANY,
            Arch::All => Arches::ALL,
            Arch::Amd64 => Arches::AMD64,
            Arch::Armel => Arches::ARMEL,
            Arch::Armhf => Arches::ARMHF,
            Arch::Arm64 => Arches::ARM64,
            Arch::I386 => Arches::I386,
            Arch::Mips => Arches::MIPS,
            Arch::Mipsel => Arches::MIPSEL,
            Arch::Mips64 => Arches::MIPS64,
            Arch::Mips64El => Arches::MIPS64EL,
            Arch::Ppc64El => Arches::PPC64EL,
            Arch::S390X => Arches::S390X,
        }
    }
}

impl FromStr for Arch {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "amd64" => Arch::Amd64,
            "armel" => Arch::Armel,
            "armhf" => Arch::Armhf,
            "arm64" => Arch::Arm64,
            "i386" => Arch::I386,
            "mips" => Arch::Mips,
            "mipsel" => Arch::Mipsel,
            "mips64" => Arch::Mips64,
            "mips64el" => Arch::Mips64El,
            "ppc64el" => Arch::Ppc64El,
            "s390x" => Arch::S390X,
            other => bail!("unrecognised arch: {:?}", s),
        })
    }
}

impl FromIterator<Arch> for Arches {
    fn from_iter<T: IntoIterator<Item = Arch>>(iter: T) -> Self {
        let mut arches = Arches::empty();
        for i in iter {
            arches |= i.as_flag()
        }

        arches
    }
}

impl Arches {
    fn parse<'a, I: IntoIterator<Item = &'a str>>(list: I) -> Result<Arches> {
        let mut val = Arches::empty();

        for part in list {
            val |= match part {
                "amd64" => Arches::AMD64,
                "armel" => Arches::ARMEL,
                "armhf" => Arches::ARMHF,
                "arm64" => Arches::ARM64,
                "i386" => Arches::I386,
                "mips" => Arches::MIPS,
                "mipsel" => Arches::MIPSEL,
                "mips64el" => Arches::MIPS64EL,
                "ppc64el" => Arches::PPC64EL,
                "s390x" => Arches::S390X,
                "any" => Arches::ANY,
                "all" => Arches::ALL,
                other => bail!("unsupported architecture: {}", other),
            };
        }

        Ok(val)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub name: String,
    pub size: u64,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vcs {
    pub description: String,
    pub type_: VcsType,
    pub tag: VcsTag,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VcsType {
    Browser,
    Arch,
    Bzr,
    Cvs,
    Darcs,
    Git,
    Hg,
    Mtn,
    Svn,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VcsTag {
    Vcs,
    Orig,
    Debian,
    Upstream,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBinary {
    pub name: String,
    pub style: String,
    pub section: String,

    pub priority: Priority,
    pub extras: Vec<String>,
}

/// https://www.debian.org/doc/debian-policy/#priorities
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Priority {
    Unknown,
    Required,
    Important,
    Standard,
    Optional,
    Extra,
    Source,
}

pub struct Description {
    pub locale: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    Unknown,
    Original,
    Quilt3dot0,
    Native3dot0,
    Git3dot0,
}

impl Package {
    pub fn parse_bin<'i, I: Iterator<Item = Result<rfc822::Line<'i>>>>(it: I) -> Result<Package> {
        use rfc822::joined;
        use rfc822::one_line;

        // Package
        let mut name = None;
        let mut version = None;
        let mut priority = None;
        let mut arch = None;
        let mut maintainer = Vec::new();
        let mut original_maintainer = Vec::new();

        // Binary
        let mut file = None;
        let mut essential = None;
        let mut build_essential = None;
        let mut installed_size = None;
        let mut description = None;
        let mut depends = Vec::new();
        let mut recommends = Vec::new();
        let mut suggests = Vec::new();
        let mut enhances = Vec::new();
        let mut pre_depends = Vec::new();
        let mut breaks = Vec::new();
        let mut conflicts = Vec::new();
        let mut replaces = Vec::new();
        let mut provides = Vec::new();

        let mut unparsed = HashMap::new();

        let mut warnings = Vec::new();

        for res in it {
            let (key, values) = res?;
            match key {
                "Package" => name = Some(one_line(&values)?),
                "Version" => version = Some(one_line(&values)?),
                "Architecture" => {
                    arch = Some(Arches::parse(one_line(&values)?
                            // TODO: alternate splitting rules?
                            .split_whitespace())?)
                }

                "Essential" => essential = Some(::yes_no(one_line(&values)?)?),
                "Build-Essential" => build_essential = Some(::yes_no(one_line(&values)?)?),
                "Priority" => priority = Some(::parse_priority(one_line(&values)?)?),
                "Maintainer" => match ::ident::read(one_line(&values)?) {
                    Ok(idents) => maintainer.extend(idents),
                    Err(e) => warnings.push(format!("parsing maintainer: {:?}", e)),
                },
                "Installed-Size" => installed_size = Some(one_line(&values)?.parse()?),
                "Description" => description = Some(joined(&values)),

                "Depends" => depends.extend(parse_dep(&values)?),
                "Recommends" => recommends.extend(parse_dep(&values)?),
                "Suggests" => suggests.extend(parse_dep(&values)?),
                "Enhances" => enhances.extend(parse_dep(&values)?),
                "Pre-Depends" => pre_depends.extend(parse_dep(&values)?),
                "Breaks" => breaks.extend(parse_dep(&values)?),
                "Conflicts" => conflicts.extend(parse_dep(&values)?),
                "Replaces" => replaces.extend(parse_dep(&values)?),
                "Provides" => provides.extend(parse_dep(&values)?),

                other => {
                    unparsed.insert(
                        other.to_string(),
                        values.iter().map(|s| s.to_string()).collect(),
                    );
                }
            }
        }

        for warning in warnings {
            eprintln!("warning in {:?} {:?}: {}", name, version, warning);
        }

        Ok(Package {
            name: name.ok_or("missing name")?.to_string(),
            version: version.ok_or("missing version")?.to_string(),
            priority: priority.ok_or("missing priority")?,
            arches: arch.ok_or("missing arch")?,
            maintainer,
            original_maintainer,
            style: PackageType::Binary(Binary {
                file,
                essential: essential.unwrap_or(false),
                build_essential: build_essential.unwrap_or(false),
                // TODO: this is missing in a couple of cases in dpkg/status; pretty crap
                installed_size: installed_size.unwrap_or(0),
                description: description.ok_or("missing description")?,
                depends,
                recommends,
                suggests,
                enhances,
                pre_depends,
                breaks,
                conflicts,
                replaces,
                provides,
            }),
            unparsed,
        })
    }
}

fn parse_dep(multi_str: &[&str]) -> Result<Vec<Dependency>> {
    deps::read(&rfc822::joined(multi_str))
}

impl Constraint {
    pub fn new(operator: ConstraintOperator, version: &str) -> Self {
        Constraint {
            operator,
            version: version.to_string(),
        }
    }

    pub fn satisfied_by<S: AsRef<str>>(&self, version: S) -> bool {
        self.operator
            .satisfied_by(compare_versions(version.as_ref(), &self.version))
    }
}

impl ConstraintOperator {
    fn satisfied_by(&self, ordering: cmp::Ordering) -> bool {
        use self::ConstraintOperator::*;
        use std::cmp::Ordering::*;

        match *self {
            Eq => Equal == ordering,
            Ge => Less != ordering,
            Le => Greater != ordering,
            Lt => Less == ordering,
            Gt => Greater == ordering,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Constraint;
    use super::ConstraintOperator;

    #[test]
    fn version() {
        let cons = Constraint::new(ConstraintOperator::Gt, "1.0");
        assert!(cons.satisfied_by("2.0"));
        assert!(!cons.satisfied_by("1.0"));
    }
}
