use std::collections::HashMap;

use errors::*;
use rfc822;

// Everything deals with streams of Items.

pub enum Item {
    End,
    Raw(RawPackage),
    Index(RawIndex),
    Package(Package),
}

// An unparsed, raw package

pub struct RawPackage {
    type_: RawPackageType,
    entries: Vec<Entry>,
}

pub enum RawPackageType {
    Source,
    Binary,
}

pub struct Entry {
    key: String,
    value: String,
}

// An unparsed, raw index

pub struct RawIndex {
    archive: String,
    version: String,
    origin: String,
    codename: String,
    label: String,
    site: String,
    component: String,
    arch: String,
    type_: String,
}

// The parsed top-level types for package

pub enum PackageType {
    Source(Source),
    Binary(Binary),
}

pub struct Package {
    name: String,
    version: String,
    priority: Priority,
    arch: Vec<String>,

    maintainer: Vec<Identity>,
    original_maintainer: Vec<Identity>,

    parse_errors: Vec<String>,
    unrecognised_fields: Vec<String>,

    style: PackageType,
}

pub struct Source {
    format: SourceFormat,

    binaries: Vec<SourceBinary>,
    files: Vec<File>,
    vcs: Vec<Vcs>,

    build_dep: Vec<Dependency>,
    build_dep_arch: Vec<Dependency>,
    build_dep_indep: Vec<Dependency>,
    build_conflict: Vec<Dependency>,
    build_conflict_arch: Vec<Dependency>,
    build_conflict_indep: Vec<Dependency>,

    uploaders: Vec<Identity>,

    unparsed: HashMap<String, String>,
}

pub struct Binary {
    file: File,

    essential: bool,
    build_essential: bool,

    installed_size: u64,

    description: String,

    depends: Vec<Dependency>,
    recommends: Vec<Dependency>,
    suggests: Vec<Dependency>,
    enhances: Vec<Dependency>,
    pre_depends: Vec<Dependency>,

    breaks: Vec<Dependency>,
    conflicts: Vec<Dependency>,
    replaces: Vec<Dependency>,

    provides: Vec<Dependency>,

    unparsed: HashMap<String, String>,
}

// The dependency chain types

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub alternate: Vec<SingleDependency>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleDependency {
    pub package: String,
    pub arch: Option<String>,
    /// Note: It's possible Debian only supports a single version constraint.
    pub version_constraints: Vec<Constraint>,
    pub arch_filter: Vec<String>,
    pub stage_filter: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    version: String,
    operator: ConstraintOperator,
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

pub struct File {
    name: String,
    size: u64,
    md5: String,
    sha1: String,
    sha256: String,
    sha512: String,
}

pub struct Vcs {
    description: String,
    type_: VcsType,
    tag: VcsTag,
}

#[derive(Copy, Clone)]
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

#[derive(Copy, Clone)]
pub enum VcsTag {
    Vcs,
    Orig,
    Debian,
    Upstream,
}

pub struct SourceBinary {
    name: String,
    style: String,
    section: String,

    priority: Priority,
    extras: Vec<String>,
}

// https://www.debian.org/doc/debian-policy/#priorities
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
    locale: String,
    value: String,
}

pub struct Identity {
    name: String,
    email: String,
}

pub enum SourceFormat {
    Unknown,
    Original,
    Quilt3dot0,
    Native3dot0,
    Git3dot0,
}

impl Package {
    fn parse_bin<'i, I: Iterator<Item = rfc822::Line<'i>>>(it: I) -> Result<Package> {
        unimplemented!()
    }
}

impl Constraint {
    pub fn new(operator: ConstraintOperator, version: &str) -> Self {
        Constraint {
            operator,
            version: version.to_string(),
        }
    }
}
