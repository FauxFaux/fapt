use std::collections::HashMap;

use mailparse::dateparse;

use errors::*;

fn label_char(c: char) -> bool {
    c.is_alphanumeric() || '-' == c
}

named!(label<&str, &str>,
    take_while1!(label_char)
);

// vec.len() (number of lines) always == 1
named!(single_line_value<&str, Vec<&str>>, do_parse!(
    line: preceded!(tag!(" "), take_until_and_consume!("\n")) >>
    ( vec![line] )
));

named!(multi_line_value<&str, Vec<&str>>,
    terminated!(
        many1!(preceded!(
            complete!(tag!("\n ")),
            take_until1!("\n"))
        ),
    tag!("\n"))
);

named!(header<&str, (&str, Vec<&str>)>, do_parse!(
    label: label >>
    tag!(":") >>
    value: alt!(single_line_value | multi_line_value) >>
    ((label, value))
));

named!(headers<&str, Vec<(&str, Vec<&str>)>>, many1!(header));

pub fn scan(block: &str) -> Result<Vec<(&str, Vec<&str>)>> {
    ensure!(block.ends_with('\n'), "tailing new line please!");
    use nom::IResult::*;
    match headers(block) {
        Done("", v) => Ok(v),
        Done(tailing, _) => bail!("trailing garbage: {:?}", tailing),
        other => bail!("other parse error: {:?}", other),
    }
}

pub fn map(block: &str) -> Result<HashMap<&str, Vec<&str>>> {
    Ok(scan(block)?.into_iter().collect())
}

#[cfg(rage)]
pub fn parse_date(date: &str) -> Result<time::Instant> {
    let epochs = dateparse(date)?;
    ensure!(epochs >= 0, "no times before the epoch");
    let secs = time::Duration::new(epochs as u64, 0);
    Ok((time::UNIX_EPOCH + secs).into())
}

pub fn parse_date(date: &str) -> Result<i64> {
    Ok(dateparse(date)?)
}


#[cfg(test)]
mod tests {
    //    use super::scan;
    use nom::IResult::*;

    #[test]
    fn single_line_header() {
        use super::header;
        assert_eq!(Done("", ("Foo", vec!["bar"])), header("Foo: bar\n"));
    }

    #[test]
    fn multi_line_header() {
        use super::header;
        assert_eq!(Done("", ("Foo", vec!["bar", "baz"])), header("Foo:\n bar\n baz\n"));
    }
}
