use errors::*;

use nom;
use nom::IResult;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dep {
    alternate: Vec<SingleDep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SingleDep {
    package: String,
    arch: String,
    version_constraints: Vec<Constraint>,
    arch_filter: Vec<String>,
    stage_filter: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Constraint {
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
enum Op {
    Ge,
    Eq,
    Le,
    Gt,
    Lt,
}

fn read(val: &str) -> Result<Vec<Dep>> {
    unimplemented!()
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
        version_constraints: many0!(version_constraint) >>
        arch_filter: many0!(arch_filter) >>
        stage_filter: many0!(stage_filter) >>
        ( SingleDep {
            package: package.to_string(),
            arch: "TODO".to_string(),
            version_constraints,
            arch_filter: arch_filter.into_iter().map(|x| x.to_string()).collect(),
            stage_filter: stage_filter.into_iter().map(|x| x.to_string()).collect(),
        } )
    ))
);


#[test]
fn check() {
    assert_eq!(IResult::Done("", "foo"), package_name("foo"));
    assert_eq!(IResult::Done(" bar", "foo"), package_name("foo bar"));

    assert_eq!(
        IResult::Done("", Constraint::new(Op::Gt, "1")),
        version_constraint("(>> 1)")
    );

    println!("{:?}", single("foo (>> 1) (<< 9) [linux-any]"))
}
