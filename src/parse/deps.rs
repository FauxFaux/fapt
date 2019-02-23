use std::cmp;
use std::collections::HashSet;

use deb_version::compare_versions;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use insideout::InsideOut;
use nom::types::CompleteStr;

use super::arch::Arch;
use super::rfc822;

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
    pub arch_filter: HashSet<(bool, Arch)>,
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

pub fn parse_dep(multi_str: &[&str]) -> Result<Vec<Dependency>, Error> {
    read(&rfc822::joined(multi_str))
}

pub fn read(val: &str) -> Result<Vec<Dependency>, Error> {
    use nom::Err as NomErr;
    match parse(CompleteStr(val)) {
        Ok((CompleteStr(""), val)) => Ok(val.into_iter().collect::<Result<_, Error>>()?),
        Err(NomErr::Incomplete(_)) => unreachable!(),
        Ok((trailing, _)) => Err(format_err!("trailing data: '{:?}'", trailing)),
        other => Err(format_err!("nom error: {:?}", other)),
    }
}

fn is_arch_char(val: char) -> bool {
    val.is_alphanumeric() || '-' == val
}

fn is_package_name_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '+' == val || '-' == val
}

fn is_version_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '~' == val || '+' == val || ':' == val || '-' == val
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

named!(package_name<CompleteStr, CompleteStr>, take_while1_s!(is_package_name_char));
named!(version<CompleteStr, CompleteStr>, take_while1_s!(is_version_char));

named!(version_constraint<CompleteStr, Constraint>,
    ws!(do_parse!(
        tag!("(") >>
        operator: alt!(
            tag!(">=") => { |_| ConstraintOperator::Ge } |
            tag!("<=") => { |_| ConstraintOperator::Le } |
            tag!(">>") => { |_| ConstraintOperator::Gt } |
            tag!("<<") => { |_| ConstraintOperator::Lt } |
            tag!(">") => { |_| ConstraintOperator::Gt } |
            tag!("<") => { |_| ConstraintOperator::Lt } |
            tag!("=") => { |_| ConstraintOperator::Eq }
        ) >>
        version: version >>
        tag!(")") >>
        ( Constraint::new(operator, version.0) )
    )));

named!(arch_part<CompleteStr, (bool, CompleteStr)>,
    do_parse!(
        bang: opt!(tag!("!")) >>
        arch: take_while1_s!(is_arch_char) >>
        ( (bang.is_none(), arch) )
    )
);

named!(arch_filter<CompleteStr, Vec<(bool, CompleteStr)>>,
    delimited!(
        tag!("["),
        ws!(many1!(ws!(arch_part))),
        tag!("]")
    )
);

named!(stage_filter<CompleteStr, CompleteStr>,
    delimited!(
        tag!("<"),
        take_until_s!(">"),
        tag!(">")
    )
);

named!(arch_suffix<CompleteStr, CompleteStr>,
    preceded!(tag!(":"), take_while1_s!(is_arch_char))
);

named!(single<CompleteStr, Result<SingleDependency, Error>>,
    ws!(do_parse!(
        package: package_name >>
        arch: opt!(complete!(arch_suffix)) >>
        version_constraints: ws!(many0!(complete!(version_constraint))) >>
        arch_filter: ws!(opt!(arch_filter)) >>
        stage_filter: ws!(many0!(complete!(stage_filter))) >>
        ( build_single_dep(package, arch, version_constraints, arch_filter, stage_filter) )
    ))
);

fn to_arch(s: CompleteStr) -> Result<Arch, Error> {
    Ok(s.parse()?)
}

fn build_single_dep(
    package: CompleteStr,
    arch: Option<CompleteStr>,
    version_constraints: Vec<Constraint>,
    arch_filter: Option<Vec<(bool, CompleteStr)>>,
    stage_filter: Vec<CompleteStr>,
) -> Result<SingleDependency, Error> {
    let package = package.to_string();
    Ok(SingleDependency {
        arch: arch
            .map(|s| to_arch(s))
            .inside_out()
            .with_context(|_| format_err!("explicit arch in dep {:?}", package))?,
        version_constraints,
        arch_filter: arch_filter
            .unwrap_or_else(Vec::new)
            .into_iter()
            .map(|(positive, arch)| to_arch(arch).map(|a| (positive, a)))
            .collect::<Result<HashSet<(bool, Arch)>, Error>>()
            .with_context(|_| format_err!("arch filter in dep {:?}", package))?,
        stage_filter: stage_filter.into_iter().map(|x| x.0.to_string()).collect(),
        package,
    })
}

named!(dep<CompleteStr, Result<Dependency, Error>>,
    ws!(do_parse!(
        alternate: ws!(separated_nonempty_list!(
            complete!(tag!("|")),
            single)
        ) >>
        ( build_dep(alternate) )
    ))
);

fn build_dep(alternate: Vec<Result<SingleDependency, Error>>) -> Result<Dependency, Error> {
    Ok(Dependency {
        alternate: alternate.into_iter().collect::<Result<_, Error>>()?,
    })
}

named!(parse<CompleteStr, Vec<Result<Dependency, Error>>>,
    ws!(
        separated_list!(
            complete!(tag!(",")),
            dep
        )
    )
);

#[test]
fn check() {
    assert_eq!(
        (CompleteStr(""), CompleteStr("foo")),
        package_name(CompleteStr("foo")).unwrap()
    );
    assert_eq!(
        (CompleteStr(" bar"), CompleteStr("foo")),
        package_name(CompleteStr("foo bar")).unwrap()
    );

    assert_eq!(
        (
            CompleteStr(""),
            Constraint::new(ConstraintOperator::Gt, "1")
        ),
        version_constraint(CompleteStr("(>> 1)")).unwrap()
    );

    println!("{:?}", single(CompleteStr("foo (>> 1) (<< 9) [linux-any]")));
    println!("{:?}", single(CompleteStr("foo [!amd64 !i386]")));
    println!("{:?}", single(CompleteStr("foo")));
    println!("{:?}", dep(CompleteStr("foo|baz")));
    println!("{:?}", dep(CompleteStr("foo | baz")));
    println!("{:?}", parse(CompleteStr("foo, baz")));

    named!(l<&str, Vec<&str>>,
        separated_nonempty_list!(complete!(tag!(",")), tag!("foo")));
    println!("{:?}", l("foo,foo"));
}

#[test]
fn constraint_version() {
    let cons = Constraint::new(ConstraintOperator::Gt, "1.0");
    assert!(cons.satisfied_by("2.0"));
    assert!(!cons.satisfied_by("1.0"));
}
