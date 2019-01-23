use failure::Error;
use nom::types::CompleteStr;

use super::types::Arch;
use super::types::Constraint;
use super::types::ConstraintOperator;
use super::types::Dependency;
use super::types::SingleDependency;

pub fn read(val: &str) -> Result<Vec<Dependency>, Error> {
    use nom::Err as NomErr;
    match parse(CompleteStr(val)) {
        Ok((CompleteStr(""), val)) => Ok(val),
        Err(NomErr::Incomplete(_)) => unreachable!(),
        Ok((trailing, _)) => Err(format_err!("trailing data: '{:?}'", trailing)),
        other => Err(format_err!("nom error: {:?}", other)),
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

named!(single<CompleteStr, SingleDependency>,
    ws!(do_parse!(
        package: package_name >>
        arch: opt!(complete!(arch_suffix)) >>
        version_constraints: ws!(many0!(complete!(version_constraint))) >>
        arch_filter: ws!(many0!(complete!(arch_filter))) >>
        stage_filter: ws!(many0!(complete!(stage_filter))) >>
        ( SingleDependency {
            package: package.0.to_string(),
            // TODO: should either validate this at the parser,
            // TODO: or work out how to propagate the error up,
            // TODO: or work out how to explain to nom that it's an error,
            // TODO: every one of these options suck
            arch: arch.map(|a| a.parse().unwrap()),
            version_constraints,
            // TODO: and here
            arch_filter: arch_filter.into_iter().map(|x| x.0.parse::<Arch>().unwrap()).collect(),
            stage_filter: stage_filter.into_iter().map(|x| x.0.to_string()).collect(),
        } )
    ))
);

named!(dep<CompleteStr, Dependency>,
    ws!(do_parse!(
        alternate: ws!(separated_nonempty_list!(
            complete!(tag!("|")),
            single)
        ) >>
        ( Dependency { alternate })
    ))
);

named!(parse<CompleteStr, Vec<Dependency>>,
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
    println!("{:?}", single(CompleteStr("foo")));
    println!("{:?}", dep(CompleteStr("foo|baz")));
    println!("{:?}", dep(CompleteStr("foo | baz")));
    println!("{:?}", parse(CompleteStr("foo, baz")));

    named!(l<&str, Vec<&str>>,
        separated_nonempty_list!(complete!(tag!(",")), tag!("foo")));
    println!("{:?}", l("foo,foo"));
}
