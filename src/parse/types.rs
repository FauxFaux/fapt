use std::cmp;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use deb_version::compare_versions;
use failure::bail;
use failure::format_err;
use failure::Error;
use insideout::InsideOut;

use super::deps;
use super::rfc822;

/// The parsed top-level types for package
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageType {
    Source(Source),
    Binary(Binary),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Ord, PartialOrd)]
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
    LinuxAny,
    X32,
}

pub type Arches = HashSet<Arch>;

impl FromStr for Arch {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match s {
            "all" => Arch::All,
            "any" => Arch::Any,
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
            "linux-any" => Arch::LinuxAny,
            "x32" => Arch::X32,
            other => bail!("unrecognised arch: {:?}", other),
        })
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Arch::All => "all",
                Arch::Any => "any",
                Arch::Amd64 => "amd64",
                Arch::Armel => "armel",
                Arch::Armhf => "armhf",
                Arch::Arm64 => "arm64",
                Arch::I386 => "i386",
                Arch::Mips => "mips",
                Arch::Mipsel => "mipsel",
                Arch::Mips64 => "mips64",
                Arch::Mips64El => "mips64el",
                Arch::Ppc64El => "ppc64el",
                Arch::S390X => "s390x",
                Arch::LinuxAny => "linux-any",
                Arch::X32 => "x32",
            }
        )
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

impl Default for Priority {
    fn default() -> Self {
        Priority::Unknown
    }
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
    pub fn parse_bin(it: rfc822::Scanner) -> Result<Package, Error> {
        let mut it = it.collect_to_map()?;
        // TODO: clearly `parse_file` is supposed to be called here somewhere
        let file = None;

        let arches = it
            .take_one_line("Architecture")?
            // TODO: alternate splitting rules?
            .split_whitespace()
            .map(|s| s.parse())
            .collect::<Result<HashSet<Arch>, Error>>()?;

        // TODO: this is missing in a couple of cases in dpkg/status; pretty crap
        let installed_size = match it.remove("Installed-Size") {
            Some(v) => rfc822::one_line(&v)?.parse()?,
            None => 0,
        };

        let essential = it
            .remove_one_line("Essential")?
            .map(|line| super::yes_no(line))
            .inside_out()?
            .unwrap_or(false);

        let build_essential = it
            .remove_one_line("Build-Essential")?
            .map(|line| super::yes_no(line))
            .inside_out()?
            .unwrap_or(false);

        Ok(Package {
            name: it.take_one_line("Package")?.to_string(),
            version: it.take_one_line("Version")?.to_string(),
            priority: super::parse_priority(it.take_one_line("Priority")?)?,
            arches,
            maintainer: super::ident::read(it.take_one_line("Maintainer")?)?,
            original_maintainer: super::ident::read(it.take_one_line("Original-Maintainer")?)?,
            style: PackageType::Binary(Binary {
                file,
                essential,
                build_essential,
                installed_size,
                description: rfc822::joined(&it.take_err("Description")?),
                depends: parse_dep(&it.remove("Depends").unwrap_or_else(Vec::new))?,
                recommends: parse_dep(&it.remove("Recommends").unwrap_or_else(Vec::new))?,
                suggests: parse_dep(&it.remove("Suggests").unwrap_or_else(Vec::new))?,
                enhances: parse_dep(&it.remove("Enhances").unwrap_or_else(Vec::new))?,
                pre_depends: parse_dep(&it.remove("Pre-Depends").unwrap_or_else(Vec::new))?,
                breaks: parse_dep(&it.remove("Breaks").unwrap_or_else(Vec::new))?,
                conflicts: parse_dep(&it.remove("Conflicts").unwrap_or_else(Vec::new))?,
                replaces: parse_dep(&it.remove("Replaces").unwrap_or_else(Vec::new))?,
                provides: parse_dep(&it.remove("Provides").unwrap_or_else(Vec::new))?,
            }),
            unparsed: it
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.into_iter().map(|v| v.to_string()).collect(),
                    )
                })
                .collect(),
        })
    }

    pub fn bin(&self) -> Option<&Binary> {
        match &self.style {
            PackageType::Binary(bin) => Some(&bin),
            _ => None,
        }
    }
}

trait MapExt {
    fn remove(&mut self, key: &str) -> Option<Vec<&str>>;

    fn take_err(&mut self, key: &str) -> Result<Vec<&str>, Error> {
        self.remove(key)
            .ok_or_else(|| format_err!("missing key: {:?}", key))
    }

    fn take_one_line(&mut self, key: &str) -> Result<&str, Error> {
        rfc822::one_line(&self.take_err(key)?)
    }

    fn remove_one_line(&mut self, key: &str) -> Result<Option<&str>, Error> {
        self.remove(key).map(|v| rfc822::one_line(&v)).inside_out()
    }
}

impl<'s> MapExt for HashMap<&'s str, Vec<&'s str>> {
    fn remove(&mut self, key: &str) -> Option<Vec<&str>> {
        HashMap::remove(self, key)
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;
        match self.arches.len() {
            0 => (),
            1 => write!(f, ":{}", self.arches.iter().next().unwrap())?,
            _ => unimplemented!("Don't know how to format multiple arches:\n{:?}", self),
        }
        write!(f, "={}", self.version)
    }
}

fn parse_dep(multi_str: &[&str]) -> Result<Vec<Dependency>, Error> {
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

impl Default for PackageType {
    fn default() -> Self {
        PackageType::Binary(Binary::default())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Constraint;
    use super::ConstraintOperator;
    use super::PackageType;

    const PROVIDES_EXAMPLE: &str = r#"Package: python3-cffi-backend
Status: install ok installed
Priority: optional
Section: python
Installed-Size: 190
Maintainer: Ubuntu Developers <ubuntu-devel-discuss@lists.ubuntu.com>
Architecture: amd64
Source: python-cffi
Version: 1.11.5-1
Replaces: python3-cffi (<< 1)
Provides: python3-cffi-backend-api-9729, python3-cffi-backend-api-max (= 10495), python3-cffi-backend-api-min (= 9729)
Depends: python3 (<< 3.7), python3 (>= 3.6~), python3:any (>= 3.1~), libc6 (>= 2.14), libffi6 (>= 3.0.4)
Breaks: python3-cffi (<< 1)
Description: Foreign Function Interface for Python 3 calling C code - runtime
 Convenient and reliable way of calling C code from Python 3.
 .
 The aim of this project is to provide a convenient and reliable way of calling
 C code from Python. It keeps Python logic in Python, and minimises the C
 required. It is able to work at either the C API or ABI level, unlike most
 other approaches, that only support the ABI level.
 .
 This package contains the runtime support for pre-built cffi modules.
Original-Maintainer: Debian Python Modules Team <python-modules-team@lists.alioth.debian.org>
Homepage: http://cffi.readthedocs.org/
"#;

    #[test]
    fn version() {
        let cons = Constraint::new(ConstraintOperator::Gt, "1.0");
        assert!(cons.satisfied_by("2.0"));
        assert!(!cons.satisfied_by("1.0"));
    }

    #[test]
    fn parse_provides() {
        let p = super::Package::parse_bin(super::rfc822::scan(PROVIDES_EXAMPLE)).unwrap();
        assert_eq!("python3-cffi-backend", p.name.as_str());
        let bin = match p.style {
            PackageType::Binary(bin) => bin,
            _ => panic!("wrong type!"),
        };
        assert_eq!(3, bin.provides.len());
    }
}
