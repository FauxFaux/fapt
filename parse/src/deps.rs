use errors::*;

use nom;
use nom::IResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    alternate: Vec<SingleDep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleDep {
    package: String,
    arch: Option<String>,
    version_constraints: Vec<Constraint>,
    arch_filter: Vec<String>,
    stage_filter: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    version: String,
    operator: Op,
}

impl Constraint {
    fn new(operator: Op, version: &str) -> Self {
        Constraint {
            operator,
            version: version.to_string(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Ge,
    Eq,
    Le,
    Gt,
    Lt,
}

pub fn read(val: &str) -> Result<Vec<Dep>> {
    let res = parse(val);

    if res.is_incomplete() {
        bail!("unexpected end of dependency line ({:?}): {}", res, val);
    }

    res.to_result().chain_err(|| format!("parsing: '{}'", val))
}

fn is_arch_char(val: char) -> bool {
    val.is_alphanumeric()
}

fn is_package_name_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '+' == val || '-' == val
}

fn is_version_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '~' == val || '+' == val || ':' == val
}

named!(package_name<&str, &str>, take_while1_s!(is_package_name_char));
named!(version<&str, &str>, take_while1_s!(is_version_char));

named!(version_constraint<&str, Constraint>,
    ws!(do_parse!(
        tag!("(") >>
        operator: alt!(
            tag!(">=") => { |_| Op::Ge } |
            tag!("==") => { |_| Op::Eq } |
            tag!("<=") => { |_| Op::Le } |
            tag!(">>") => { |_| Op::Gt } |
            tag!("<<") => { |_| Op::Lt }
        ) >>
        version: version >>
        tag!(")") >>
        ( Constraint::new(operator, version) )
    )));

named!(arch_filter<&str, &str>,
    delimited!(
        tag!("["),
        take_until_s!("]"),
        tag!("]")
    )
);

named!(stage_filter<&str, &str>,
    delimited!(
        tag!("<"),
        take_until_s!(">"),
        tag!("<")
    )
);

named!(single<&str, SingleDep>,
    ws!(do_parse!(
        package: package_name >>
        arch: opt!(preceded!(tag!(":"), take_while1_s!(is_arch_char))) >>
        version_constraints: ws!(many0!(version_constraint)) >>
        arch_filter: ws!(many0!(arch_filter)) >>
        stage_filter: ws!(many0!(stage_filter)) >>
        ( SingleDep {
            package: package.to_string(),
            arch: arch.map(|x| x.to_string()),
            version_constraints,
            arch_filter: arch_filter.into_iter().map(|x| x.to_string()).collect(),
            stage_filter: stage_filter.into_iter().map(|x| x.to_string()).collect(),
        } )
    ))
);

named!(dep<&str, Dep>,
    ws!(do_parse!(
        alternate: ws!(separated_nonempty_list!(tag!("|"), single)) >>
        ( Dep { alternate })
    ))
);

named!(parse<&str, Vec<Dep>>, ws!(separated_list!(tag!(","), dep)));

#[test]
fn check() {
    assert_eq!(IResult::Done("", "foo"), package_name("foo"));
    assert_eq!(IResult::Done(" bar", "foo"), package_name("foo bar"));

    assert_eq!(
        IResult::Done("", Constraint::new(Op::Gt, "1")),
        version_constraint("(>> 1)")
    );

    println!("{:?}", single("foo (>> 1) (<< 9) [linux-any]"));
    println!("{:?}", single("foo"));
    println!("{:?}", dep("foo|baz"));
    println!("{:?}", dep("foo | baz"));
    println!("{:?}", parse("foo, baz"));
}
