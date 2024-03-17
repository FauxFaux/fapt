use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use nom::bytes::complete::{tag, take_until};
use nom::{Err, IResult};

/// A user identity, e.g. `John Smith <john@smi.th>`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

fn ident(from: &str) -> IResult<&str, Result<Identity, Error>> {
    let (from, name) = take_until(" <")(from)?;
    let (from, _) = tag(" <")(from)?;
    println!("name: {:?}", name);
    print!("from: {:?}", from);
    let (from, email) = take_until(">")(from)?;
    let (from, _) = tag(">")(from)?;
    let identity = process_escapes(name.trim()).map(|name| Identity {
        name: name.to_string(),
        email: email.to_string(),
    });

    Ok((from, identity))
}

named!(parse<&str, Vec<Result<Identity, Error>>>,
    ws!(
        terminated!(
            separated_list!(
                complete!(tag!(",")),
                complete!(ident)
            ),
            opt!(complete!(tag!(",")))
        )
    )
);

pub fn read(from: &str) -> Result<Vec<Identity>, Error> {
    match parse(from) {
        Ok(("", vec)) => vec.into_iter().collect::<Result<Vec<Identity>, Error>>(),
        Ok((tailing, _)) => bail!(
            "parsing {:?} finished early, trailing garbage: {:?}",
            from,
            tailing
        ),
        Err(Err::Incomplete(_)) => unreachable!(),
        other => bail!("parsing {:?} failed: {:?}", from, other),
    }
}

fn process_escapes(from: &str) -> Result<String, Error> {
    let mut bytes = from.bytes();
    let mut result = Vec::with_capacity(bytes.len());
    loop {
        match bytes.next() {
            Some(c) if b'\\' == c => match bytes.next() {
                Some(c) if [b'\'', b'"'].contains(&c) => result.push(c),
                Some(c) if b'x' == c => result.push(parse_ascii_hex(
                    bytes
                        .next()
                        .ok_or_else(|| anyhow!("\\x must be followed by a character"))?,
                    bytes
                        .next()
                        .ok_or_else(|| anyhow!("\\xX must be followed"))?,
                )?),
                Some(c) => bail!("unsupported escape: {:?}", c),
                None => bail!("\\ at end of string"),
            },
            Some(c) => result.push(c),
            None => return Ok(String::from_utf8(result)?),
        }
    }
}

fn parse_ascii_hex(first: u8, second: u8) -> Result<u8, Error> {
    // Bit ugly, but at least it doesn't involve any code
    Ok(u8::from_str_radix(
        &String::from_utf8(vec![first, second])?,
        16,
    )?)
}

#[cfg(test)]
mod tests {
    use crate::parse::ident::ident;

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

        let (rem, res) = ident("foo <bar>").unwrap();
        assert_eq!("", rem);
        assert_eq!(
            Identity {
                name: "foo".to_string(),
                email: "bar".to_string(),
            },
            res.unwrap()
        );

        assert_eq!(
            vec![
                Identity {
                    name: "foo".to_string(),
                    email: "bar".to_string(),
                },
                Identity {
                    name: "baz".to_string(),
                    email: "quux".to_string(),
                },
            ],
            read("foo <bar>, baz <quux>").unwrap()
        );
    }

    #[test]
    fn lazy() {
        use super::read;
        assert!(read("just@email.com").is_err());
    }

    #[test]
    fn trailing() {
        use super::read;
        assert_eq!(1, read("foo <bar>,").unwrap().len())
    }
}
