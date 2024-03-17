use std::cmp;
use std::collections::HashSet;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use deb_version::compare_versions;
use insideout::InsideOut;
use nom::branch::alt;
use nom::bytes::complete;
use nom::bytes::complete::{tag, take_while};
use nom::character::complete::multispace0;
use nom::combinator::opt;
use nom::multi::{many0, many1, separated_nonempty_list};
use nom::sequence::delimited;
use nom::IResult;

use super::arch::Arch;
use crate::rfc822;

/// One-or-more alternate dependencies from a dependency list. e.g. `foo (>2.1) | bar [!i386 !amd64]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub alternate: Vec<SingleDependency>,
}

/// A dependency specification, e.g. `foo (>2.1) [!linux] <first>`
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SingleDependency {
    pub package: String,
    pub arch: Option<Arch>,
    /// Note: It's possible Debian only supports a single version constraint.
    pub version_constraints: Vec<Constraint>,
    pub arch_filter: HashSet<(bool, Arch)>,
    pub stage_filter: Vec<String>,
}

/// A constraint on a version, e.g. `>2.1`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub version: String,
    pub operator: ConstraintOperator,
}

/// An operator inside a constraint, e.g. `>`, `<`, `<=`, ...
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
    match parse(val) {
        Ok(("", val)) => Ok(val.into_iter().collect::<Result<_, Error>>()?),
        Ok((trailing, _)) => Err(anyhow!("trailing data: '{:?}'", trailing)),
        other => Err(anyhow!("nom error: {:?}", other)),
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

fn package_name(i: &str) -> IResult<&str, &str> {
    complete::take_while1(is_package_name_char)(i)
}

fn version(i: &str) -> IResult<&str, &str> {
    complete::take_while1(is_version_char)(i)
}

fn version_constraint(i: &str) -> IResult<&str, Constraint> {
    let (i, _) = multispace0(i)?;
    let (i, _) = complete::tag("(")(i)?;
    let (i, operator) = alt((
        complete::tag(">="),
        complete::tag("<="),
        complete::tag(">>"),
        complete::tag("<<"),
        complete::tag(">"),
        complete::tag("<"),
        complete::tag("="),
    ))(i)?;
    let (i, _) = many0(complete::tag(" "))(i)?;
    let operator = match operator {
        ">=" => ConstraintOperator::Ge,
        "<=" => ConstraintOperator::Le,
        ">>" => ConstraintOperator::Gt,
        "<<" => ConstraintOperator::Lt,
        ">" => ConstraintOperator::Gt,
        "<" => ConstraintOperator::Lt,
        "=" => ConstraintOperator::Eq,
        _ => unreachable!(),
    };
    let (i, version) = version(i)?;
    let (i, _) = complete::tag(")")(i)?;
    Ok((i, Constraint::new(operator, version)))
}

fn arch_part(i: &str) -> IResult<&str, (bool, &str)> {
    let (i, bang) = opt(complete::tag("!"))(i)?;
    let (i, arch) = complete::take_while1(is_arch_char)(i)?;
    Ok((i, (bang.is_none(), arch)))
}

fn arch_filter(i: &str) -> IResult<&str, Vec<(bool, &str)>> {
    let (i, _) = complete::tag("[")(i)?;
    let (i, arches) = many1(arch_part)(i)?;
    let (i, _) = complete::tag("]")(i)?;
    Ok((i, arches))
}

fn stage_filter(i: &str) -> IResult<&str, &str> {
    let (i, _) = complete::tag("<")(i)?;
    let (i, stage) = complete::take_until(">")(i)?;
    let (i, _) = complete::tag(">")(i)?;
    Ok((i, stage))
}

fn arch_suffix(i: &str) -> IResult<&str, &str> {
    let (i, _) = complete::tag(":")(i)?;
    complete::take_while1(is_arch_char)(i)
}

fn single(i: &str) -> IResult<&str, Result<SingleDependency, Error>> {
    let (i, package) = package_name(i)?;
    let (i, _) = multispace0(i)?;
    let (i, arch) = opt(arch_suffix)(i)?;
    let (i, _) = multispace0(i)?;
    let (i, version_constraints) = many0(version_constraint)(i)?;
    let (i, _) = multispace0(i)?;
    let (i, arch_filter) = opt(arch_filter)(i)?;
    let (i, _) = multispace0(i)?;
    let (i, stage_filter) = many0(stage_filter)(i)?;
    Ok((
        i,
        build_single_dep(
            package,
            arch,
            version_constraints,
            arch_filter,
            stage_filter,
        ),
    ))
}

fn to_arch(s: &str) -> Result<Arch, Error> {
    Ok(s.parse()?)
}

fn build_single_dep(
    package: &str,
    arch: Option<&str>,
    version_constraints: Vec<Constraint>,
    arch_filter: Option<Vec<(bool, &str)>>,
    stage_filter: Vec<&str>,
) -> Result<SingleDependency, Error> {
    let package = package.to_string();
    Ok(SingleDependency {
        arch: arch
            .map(|s| to_arch(s))
            .inside_out()
            .with_context(|| anyhow!("explicit arch in dep {:?}", package))?,
        version_constraints,
        arch_filter: arch_filter
            .unwrap_or_else(Vec::new)
            .into_iter()
            .map(|(positive, arch)| to_arch(arch).map(|a| (positive, a)))
            .collect::<Result<HashSet<(bool, Arch)>, Error>>()
            .with_context(|| anyhow!("arch filter in dep {:?}", package))?,
        stage_filter: stage_filter.into_iter().map(|x| x.to_string()).collect(),
        package,
    })
}

fn dep(i: &str) -> IResult<&str, Result<Dependency, Error>> {
    let (i, _) = multispace0(i)?;
    let (i, alternate) =
        separated_nonempty_list(delimited(multispace0, tag("|"), multispace0), single)(i)?;
    Ok((i, build_dep(alternate)))
}

fn build_dep(alternate: Vec<Result<SingleDependency, Error>>) -> Result<Dependency, Error> {
    Ok(Dependency {
        alternate: alternate.into_iter().collect::<Result<_, Error>>()?,
    })
}

named!(parse<&str, Vec<Result<Dependency, Error>>>,
    ws!(
        separated_list!(
            complete!(tag!(",")),
            dep
        )
    )
);

#[test]
fn check() {
    assert_eq!(("", "foo"), package_name("foo").unwrap());
    assert_eq!((" bar", "foo"), package_name("foo bar").unwrap());

    assert_eq!(
        ("", Constraint::new(ConstraintOperator::Gt, "1")),
        version_constraint("(>> 1)").unwrap()
    );

    assert_eq!("foo", package_name("foo").unwrap().1);
    assert_eq!("1", version("1").unwrap().1);
    assert_eq!((false, "foo"), arch_part("!foo").unwrap().1);
    assert_eq!((true, "foo"), arch_part("foo").unwrap().1);
    assert_eq!(
        Constraint {
            version: "1".to_string(),
            operator: ConstraintOperator::Gt
        },
        version_constraint("(>> 1)").unwrap().1
    );

    println!("{:?}", single("foo (>> 1) (<< 9) [linux-any]"));
    println!("{:?}", single("foo [!amd64 !i386]"));
    println!("{:?}", single("foo"));
    println!("{:?}", dep("foo|baz"));
    println!("{:?}", dep("foo | baz"));
    println!("{:?}", parse("foo, baz"));

    named!(l<&str, Vec<&str>>,
        separated_nonempty_list!(complete!(tag!(",")), tag!("foo")));
    println!("{:?}", l("foo,foo"));
}

#[test]
fn bare_single() {
    let (rem, res) = single("foo").unwrap();
    assert_eq!("foo", res.unwrap().package);
    assert_eq!("", rem);
}

#[test]
fn constraint_version() {
    let cons = Constraint::new(ConstraintOperator::Gt, "1.0");
    assert!(cons.satisfied_by("2.0"));
    assert!(!cons.satisfied_by("1.0"));
}
