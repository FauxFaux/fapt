use nom::IResult;

use errors::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

fn email_user_char(c: char) -> bool {
    '@' != c && !c.is_whitespace()
}

fn email_domain_char(c: char) -> bool {
    '<' != c && !c.is_whitespace()
}

// [^ ]+@[^ ]+
named!(email<&str, String>,
    do_parse!(
        user: complete!(take_while1_s!(email_user_char)) >>
        tag!("@") >>
        domain: take_while1_s!(email_domain_char) >>
        ( format!("{}@{}", user, domain) )
    )
);

named!(email_only_ident<&str, Result<Identity>>,
    do_parse!(
        email: email >>
        (
            Ok(Identity {
                name: String::new(),
                email: email.to_string(),
            })
        )
    )
);

named!(ident<&str, Result<Identity>>,
    do_parse!(
        name: take_until_and_consume_s!(" <") >>
        email: take_until_and_consume_s!(">") >>
        ( process_escapes(name.trim()).map(|name|
            Identity {
                name: name.to_string(),
                email: email.to_string(),
            })
        )
    )
);

named!(parse<&str, Vec<Result<Identity>>>,
    ws!(
        terminated!(
            separated_list!(
                complete!(tag!(",")),
                alt_complete!(ident | email_only_ident)
            ),
            opt!(complete!(tag!(",")))
        )
    )
);

pub fn read(from: &str) -> Result<Vec<Identity>> {
    match parse(from) {
        IResult::Done("", vec) => vec.into_iter().collect::<Result<Vec<Identity>>>(),
        other => bail!("parsing failed: {:?}", other),
    }
}

fn process_escapes(from: &str) -> Result<String> {
    let mut bytes = from.bytes();
    let mut result = Vec::with_capacity(bytes.len());
    loop {
        match bytes.next() {
            Some(c) if b'\\' == c => {
                match bytes.next() {
                    Some(c) if [b'\'', b'"'].contains(&c) => result.push(c),
                    Some(c) if b'x' == c => {
                        result.push(parse_ascii_hex(
                            bytes.next().ok_or("\\x must be followed by a character")?,
                            bytes.next().ok_or("\\xX must be followed")?,
                        )?)
                    }
                    Some(c) => bail!("unsupported escape: {:?}", c),
                    None => bail!("\\ at end of string"),
                }
            }
            Some(c) => result.push(c),
            None => return Ok(String::from_utf8(result)?),
        }
    }
}

fn parse_ascii_hex(first: u8, second: u8) -> Result<u8> {
    // Bit ugly, but at least it doesn't involve any code
    Ok(u8::from_str_radix(
        &String::from_utf8(vec![first, second])?,
        16,
    )?)
}


#[cfg(test)]
mod tests {
    #[test]
    fn backslash() {
        use super::process_escapes;
        assert_eq!("foo", &process_escapes("foo").unwrap());
        assert_eq!("fo'o", &process_escapes("fo\\'o").unwrap());
        assert_eq!("fo'", &process_escapes("fo\\'").unwrap());
        assert!(process_escapes("fo\\").is_err());
        assert!(process_escapes("fo\\a").is_err());

        assert_eq!("a", &process_escapes("\\x61").unwrap());
    }

    #[test]
    fn read() {
        use super::read;
        use super::Identity;

        assert_eq!(
            vec![
                Identity { name: "foo".to_string(), email: "bar".to_string() },
                Identity { name: "baz".to_string(), email: "quux".to_string() },
            ],
            read("foo <bar>, baz <quux>").unwrap()
        );
    }

    #[test]
    fn trailing() {
        use super::read;
        assert_eq!(1, read("foo <bar>,").unwrap().len())
    }
}
