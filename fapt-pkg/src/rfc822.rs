use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::BufRead;

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
    tag!(" ") >>
    line: take_until1!("\n") >>
    ( vec![line] )
));

named!(multi_line_tailing<&str, Vec<&str>>,
    many1!(preceded!(
        complete!(tag!("\n ")),
        take_until1!("\n"))
    )
);

named!(header<&str, (&str, Vec<&str>)>, do_parse!(
    label: label >>
    tag!(":") >>
    value: alt!(single_line_value | multi_line_tailing) >>
    tag!("\n") >>
    ((label, value))
));

named!(headers<&str, Vec<(&str, Vec<&str>)>>, many1!(header));

pub fn scan(block: &str) -> Result<Vec<(&str, Vec<&str>)>> {
    ensure!(block.ends_with('\n'), "tailing new line please!");
    use nom::IResult::*;
    match headers(block) {
        Done("", v) => Ok(v),
        Done(tailing, _) => bail!("trailing garbage in block: {:?}", tailing),
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

pub struct Section<R: Read> {
    from: io::BufReader<R>,
}

impl<R: Read> Section<R> {
    pub fn new(from: R) -> Self {
        Section { from: io::BufReader::new(from) }
    }
}

impl<R: Read> Iterator for Section<R> {
    type Item = Result<Vec<u8>>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::with_capacity(8 * 1024);

        // while can read non-blank lines, stuff them in the buf
        while match self.from.read_until(b'\n', &mut buf) {
            Ok(size) => size,
            Err(e) => return Some(Err(e.into())),
        } > 1
        {}

        if buf.is_empty() {
            None
        } else {
            // double new-line on the end, from a normal parse
            if b'\n' == buf[buf.len() - 2] {
                buf.pop();
            }
            Some(Ok(buf))
        }
    }
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

    #[test]
    fn multi_line_joined() {
        use super::header;
        assert_eq!(Done("", ("Foo", vec!["bar", "baz", "quux"])), header("Foo: bar\n bar\n quux\n"));
    }

    #[test]
    fn walkies() {
        use std::io;
        use super::Section;
        use errors::*;

        let parts: Result<Vec<Vec<u8>>> = Section::new(io::Cursor::new(b"foo\nbar\n\nbaz\n"))
            .collect();
        assert_eq!(vec![
            b"foo\nbar\n".to_vec(),
            b"baz\n".to_vec()
        ], parts.unwrap());
    }
}
