use nom::Err;
use nom::types::CompleteStr;

use errors::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    pub alternate: Vec<SingleDep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleDep {
    pub package: String,
    pub arch: Option<String>,
    pub version_constraints: Vec<Constraint>,
    pub arch_filter: Vec<String>,
    pub stage_filter: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    pub version: String,
    pub operator: Op,
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
    use nom::Err as NomErr;
    match parse(CompleteStr(val)) {
        Ok((CompleteStr(""), val)) => Ok(val),
        Err(NomErr::Incomplete(_)) => unreachable!(),
        Ok((trailing, _)) => Err(format!("trailing data: '{:?}'", trailing).into()),
        other => Err(format!("nom error: {:?}", other).into()),
    }
}

fn is_arch_char(val: char) -> bool {
    val.is_alphanumeric()
}

fn is_package_name_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '+' == val || '-' == val
}

fn is_version_char(val: char) -> bool {
    val.is_alphanumeric() || '.' == val || '~' == val || '+' == val || ':' == val || '-' == val
}

named!(package_name<CompleteStr, CompleteStr>, take_while1_s!(is_package_name_char));
named!(version<CompleteStr, CompleteStr>, take_while1_s!(is_version_char));

named!(version_constraint<CompleteStr, Constraint>,
    ws!(do_parse!(
        tag!("(") >>
        operator: alt!(
            tag!(">=") => { |_| Op::Ge } |
            tag!("<=") => { |_| Op::Le } |
            tag!(">>") => { |_| Op::Gt } |
            tag!("<<") => { |_| Op::Lt } |
            tag!(">") => { |_| Op::Gt } |
            tag!("<") => { |_| Op::Lt } |
            tag!("=") => { |_| Op::Eq }
        ) >>
        version: version >>
        tag!(")") >>
        ( Constraint::new(operator, version.0) )
    )));

named!(arch_filter<CompleteStr, CompleteStr>,
    delimited!(
        tag!("["),
        take_until_s!("]"),
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

named!(single<CompleteStr, SingleDep>,
    ws!(do_parse!(
        package: package_name >>
        arch: opt!(complete!(arch_suffix)) >>
        version_constraints: ws!(many0!(complete!(version_constraint))) >>
        arch_filter: ws!(many0!(complete!(arch_filter))) >>
        stage_filter: ws!(many0!(complete!(stage_filter))) >>
        ( SingleDep {
            package: package.0.to_string(),
            arch: arch.map(|x| x.0.to_string()),
            version_constraints,
            arch_filter: arch_filter.into_iter().map(|x| x.0.to_string()).collect(),
            stage_filter: stage_filter.into_iter().map(|x| x.0.to_string()).collect(),
        } )
    ))
);

named!(dep<CompleteStr, Dep>,
    ws!(do_parse!(
        alternate: ws!(separated_nonempty_list!(
            complete!(tag!("|")),
            single)
        ) >>
        ( Dep { alternate })
    ))
);

named!(parse<CompleteStr, Vec<Dep>>,
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
        (CompleteStr(""), Constraint::new(Op::Gt, "1")),
        version_constraint(CompleteStr("(>> 1)")).unwrap()
    );

    println!("{:?}", single(CompleteStr("foo (>> 1) (<< 9) [linux-any]")));
    println!("{:?}", single(CompleteStr("foo")));
    println!("{:?}", dep(CompleteStr("foo|baz")));
    println!("{:?}", dep(CompleteStr("foo | baz")));
    println!("{:?}", parse(CompleteStr("foo, baz")));

    named!(l<&str, Vec<&str>>,
        separated_nonempty_list!(complete!(tag!(",")), tag!("foo")));
    println!("{:?}", l("foo,foo"));
}
