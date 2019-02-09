use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::iter::Peekable;
use std::str::Lines;

use chrono::DateTime;
use chrono::Utc;
use failure::ensure;
use failure::err_msg;
use failure::format_err;
use failure::Error;
use failure::ResultExt;

pub type Line<'s> = (&'s str, Vec<&'s str>);

pub fn scan(block: &str) -> Scanner {
    Scanner {
        it: block.lines().peekable(),
    }
}

#[derive(Clone, Debug)]
pub struct Scanner<'a> {
    it: Peekable<Lines<'a>>,
}

impl<'a> Scanner<'a> {
    pub fn collect_to_map(self) -> Result<HashMap<&'a str, Vec<&'a str>>, Error> {
        let mut ret = HashMap::with_capacity(16);
        for val in self {
            let (key, val) = val?;
            ret.insert(key, val);
        }
        Ok(ret)
    }

    pub fn find_key(self, key: &str) -> Result<Option<Vec<&'a str>>, Error> {
        for line in self {
            let (this_key, value) = line?;
            if this_key == key {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Line<'a>, Error>;

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
        let mut sub = Vec::new();
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

pub fn parse_date(date: &str) -> Result<DateTime<Utc>, Error> {
    // TODO: SIGH
    let fixed;
    let mut f;
    if date.ends_with(" UTC") {
        f = date[..date.len() - " UTC".len()].to_string();
        f.push_str(" +0000");
        fixed = f.as_str();
    } else {
        fixed = date;
    }

    Ok(
        chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc2822(fixed)
            .with_context(|_| format_err!("parsing {:?} as date", date))?
            .with_timezone(&chrono::offset::Utc),
    )
}

pub struct Section<R: Read> {
    from: io::BufReader<R>,
}

impl<R: Read> Section<R> {
    pub fn new(from: R) -> Self {
        Section {
            from: io::BufReader::new(from),
        }
    }
}

impl<R: Read> Iterator for Section<R> {
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

pub fn mandatory_single_line(data: &HashMap<&str, Vec<&str>>, key: &str) -> Result<String, Error> {
    Ok(data
        .get(key)
        .ok_or_else(|| format_err!("{} is mandatory", key))?
        .join(" "))
}

pub fn mandatory_whitespace_list(
    data: &HashMap<&str, Vec<&str>>,
    key: &str,
) -> Result<Vec<String>, Error> {
    Ok(mandatory_single_line(data, key)?
        .split_whitespace()
        .map(|x| x.to_string())
        .collect())
}

pub fn one_line<'a>(lines: &[&'a str]) -> Result<&'a str, Error> {
    ensure!(1 == lines.len(), "{:?} isn't exactly one line", lines);
    Ok(lines[0])
}

pub fn joined(lines: &[&str]) -> String {
    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use super::parse_date;
    use super::scan;
    use super::Line;

    #[test]
    fn single_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar"])],
            scan("Foo: bar\n")
                .collect::<Result<Vec<Line>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn multi_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz"])],
            scan("Foo:\n bar\n baz\n")
                .collect::<Result<Vec<Line>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn multi_line_joined() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz", "quux"])],
            scan("Foo: bar\n baz\n quux\n")
                .collect::<Result<Vec<Line>, Error>>()
                .unwrap()
        );
    }

    #[test]
    fn walkies() {
        use super::Section;
        use std::io;

        let parts: Result<Vec<Vec<u8>>, Error> =
            Section::new(io::Cursor::new(b"foo\nbar\n\nbaz\n")).collect();
        assert_eq!(
            vec![b"foo\nbar\n".to_vec(), b"baz\n".to_vec()],
            parts.unwrap()
        );
    }

    #[test]
    fn date_parsing_seriously_it_is_2019() {
        use chrono::Timelike;
        assert_eq!(
            14,
            parse_date("Wed, 06 Feb 2019 14:29:43 UTC").unwrap().hour()
        );
    }
}
