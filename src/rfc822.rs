//! Low-level access to `deb822` files.
//!
//! `deb822` is a spec-violating extension to `rfc822`, the email format.

use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::iter::Peekable;
use std::str::Lines;

use chrono::DateTime;
use chrono::Utc;
use failure::ensure;
use failure::format_err;
use failure::Error;
use failure::ResultExt;
use insideout::InsideOut;

/// A _Field_ from a _Block_, consisting of a _Key_ and a list of one-or-more lines.
pub type Field<'s> = (&'s str, Vec<&'s str>);

/// A mapping from _Key_ to one-or-more lines.
pub type Map<'s> = HashMap<&'s str, Vec<&'s str>>;

/// Produce an iterator over the _Fields_ in a _Block_.
pub fn fields_in_block(block: &str) -> Fields {
    Fields {
        it: block.lines().peekable(),
    }
}

/// Iterate over the _Fields_ in a _Block_.
#[derive(Clone, Debug)]
pub struct Fields<'a> {
    it: Peekable<Lines<'a>>,
}

impl<'a> Fields<'a> {
    pub fn collect_to_map(self) -> Result<Map<'a>, Error> {
        let mut ret = HashMap::with_capacity(32);
        for val in self {
            let (key, val) = val?;
            ret.insert(key, val);
        }
        Ok(ret)
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<Field<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = match self.it.next() {
            Some(line) => line,
            None => return None,
        };

        let colon = match line.find(':') {
            Some(colon) => colon,
            None => return Some(Err(format_err!("expected a key: in {:?}", line))),
        };

        let (key, first_val) = line.split_at(colon);
        let first_val = first_val[1..].trim();
        let mut sub = Vec::with_capacity(8);
        if !first_val.is_empty() {
            sub.push(first_val);
        }

        loop {
            match self.it.peek() {
                Some(line) if line.starts_with(' ') => {
                    sub.push(line.trim());
                }
                Some(_) | None => break,
            }

            self.it.next().expect("just peeked");
        }

        Some(Ok((key, sub)))
    }
}

pub(crate) fn parse_date(date: &str) -> Result<DateTime<Utc>, Error> {
    use chrono::offset::TimeZone;
    let signed_epoch = mailparse::dateparse(date)
        .map_err(|msg| format_err!("parsing {:?} as date: {}", date, msg))?;
    Ok(chrono::Utc.timestamp(signed_epoch, 0))
}

pub(crate) struct ByteBlocks<R> {
    pub(crate) name: String,
    from: io::BufReader<R>,
}

impl<R: Read> ByteBlocks<R> {
    pub fn new(from: R, name: String) -> Self {
        ByteBlocks {
            name,
            from: io::BufReader::new(from),
        }
    }

    pub fn into_string_blocks(self) -> Blocks<R> {
        Blocks { inner: self }
    }
}

impl<R: Read> Iterator for ByteBlocks<R> {
    type Item = Result<Vec<u8>, Error>;

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

/// An iterator over _Blocks_.
pub struct Blocks<R> {
    pub(crate) inner: ByteBlocks<R>,
}

impl<R: Read> Blocks<R> {
    pub fn new(from: R, name: String) -> Self {
        ByteBlocks::new(from, name).into_string_blocks()
    }
}

impl<R: Read> Iterator for Blocks<R> {
    type Item = Result<String, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|v| v.and_then(|v| -> Result<String, Error> { Ok(String::from_utf8(v)?) }))
    }
}

fn one_line<'a>(lines: &[&'a str]) -> Result<&'a str, Error> {
    ensure!(1 == lines.len(), "{:?} isn't exactly one line", lines);
    Ok(lines[0])
}

pub(crate) fn joined(lines: &[&str]) -> String {
    lines.join(" ")
}

/// Helpers for dealing with an `Map`.
pub trait RfcMapExt {
    fn get(&self, key: &str) -> Option<&Vec<&str>>;
    fn remove(&mut self, key: &str) -> Option<Vec<&str>>;

    fn get_value<'k, 'u>(&'u self, key: &'k str) -> Value<'k, &[&'u str]> {
        Value {
            key,
            val: self.get(key).map(|v| v.as_slice()),
        }
    }

    fn remove_value<'k, 'u>(&'u mut self, key: &'k str) -> Value<'k, Vec<&'u str>> {
        Value {
            key,
            val: self.remove(key),
        }
    }
}

impl<'s> RfcMapExt for HashMap<&'s str, Vec<&'s str>> {
    fn get(&self, key: &str) -> Option<&Vec<&str>> {
        HashMap::get(self, key)
    }
    fn remove(&mut self, key: &str) -> Option<Vec<&str>> {
        HashMap::remove(self, key)
    }
}

/// An optional list-of-lines from a `Map`, which carries
/// the original _Key_ along, for error reporting purposes.
pub struct Value<'k, T> {
    pub key: &'k str,
    pub val: Option<T>,
}

impl<'k, 's, T: AsRef<[&'s str]>> Value<'k, T> {
    pub fn required(&self) -> Result<&[&'s str], Error> {
        Ok(self
            .val
            .as_ref()
            .ok_or_else(|| format_err!("{:?} required", self.key))?
            .as_ref())
    }

    pub fn one_line(&self) -> Result<Option<&'s str>, Error> {
        Ok(self
            .val
            .as_ref()
            .map(|lines| one_line(lines.as_ref()))
            .inside_out()
            .with_context(|_| format_err!("{:?} should be one line", self.key))?)
    }

    pub fn one_line_owned(&self) -> Result<Option<String>, Error> {
        Ok(self.one_line()?.map(ToString::to_string))
    }

    pub fn one_line_req(&self) -> Result<&'s str, Error> {
        self.one_line()
            .and_then(|o| o.ok_or_else(|| format_err!("{:?} required", self.key)))
    }

    pub fn joined_lines(&self) -> Option<String> {
        self.val.as_ref().map(|lines| joined(lines.as_ref()))
    }

    pub fn joined_lines_req(&self) -> Result<String, Error> {
        self.joined_lines()
            .ok_or_else(|| format_err!("{:?} required", self.key))
    }

    pub fn split_comma(&self) -> Result<Vec<&'s str>, Error> {
        let lines = self.required()?;
        let mut ret = Vec::with_capacity(lines.len() * 8);
        for line in lines {
            for word in line.split(',') {
                let stripped = word.trim();
                if stripped.is_empty() {
                    continue;
                }
                ret.push(stripped);
            }
        }
        Ok(ret)
    }

    pub fn split_whitespace(&self) -> Result<Vec<String>, Error> {
        let lines = self.required()?;
        let mut ret = Vec::with_capacity(lines.len() * 8);
        for line in lines {
            for word in line.split_whitespace() {
                ret.push(word.to_string());
            }
        }
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use super::fields_in_block;
    use super::parse_date;
    use super::Field;

    #[test]
    fn single_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar"])],
            fields_in_block("Foo: bar\n")
                .collect::<Result<Vec<Field>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn multi_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz"])],
            fields_in_block("Foo:\n bar\n baz\n")
                .collect::<Result<Vec<Field>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn multi_line_joined() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz", "quux"])],
            fields_in_block("Foo: bar\n baz\n quux\n")
                .collect::<Result<Vec<Field>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn walkies() {
        use super::ByteBlocks;
        use std::io;

        let parts: Result<Vec<Vec<u8>>, Error> =
            ByteBlocks::new(io::Cursor::new(b"foo\nbar\n\nbaz\n"), String::new()).collect();
        assert_eq!(
            vec![b"foo\nbar\n".to_vec(), b"baz\n".to_vec()],
            parts.unwrap()
        );
    }

    #[test]
    fn date_parsing_seriously_it_is_2019() {
        use chrono::Datelike;
        use chrono::Timelike;
        let d = parse_date("Wed, 06 Feb 2019 14:29:43 UTC").unwrap();
        assert_eq!(
            (2019, 2, 6, 14, 29, 43),
            (
                d.year(),
                d.month(),
                d.day(),
                d.hour(),
                d.minute(),
                d.second()
            ),
        );

        // .. single digit hours? That is literally impossible. How could that happen?
        let d = parse_date("Wed, 13 Feb 2019  6:51:09 UTC").unwrap();
        assert_eq!(
            (2019, 2, 13, 6, 51, 9),
            (
                d.year(),
                d.month(),
                d.day(),
                d.hour(),
                d.minute(),
                d.second()
            ),
        );
    }
}
