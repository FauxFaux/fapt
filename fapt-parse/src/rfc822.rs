use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::iter::Peekable;
use std::str::Lines;

use mailparse::dateparse;

use errors::*;

pub type Line<'s> = (&'s str, Vec<&'s str>);

pub fn scan(block: &str) -> impl Iterator<Item = Result<Line>> {
    Scanner {
        it: block.lines().peekable(),
    }
}

struct Scanner<'a> {
    it: Peekable<Lines<'a>>,
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Line<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = match self.it.next() {
            Some(line) => line,
            None => return None,
        };

        let colon = match line.find(':') {
            Some(colon) => colon,
            None => return Some(Err(format!("expected a key: in {:?}", line).into())),
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

pub fn map(block: &str) -> Result<HashMap<&str, Vec<&str>>> {
    // Vec collect() hack doesn't seem to apply to map; super lazy solution
    Ok(scan(block)
        .collect::<Result<Vec<Line>>>()?
        .into_iter()
        .collect())
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
        Section {
            from: io::BufReader::new(from),
        }
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

pub fn mandatory_single_line(data: &HashMap<&str, Vec<&str>>, key: &str) -> Result<String> {
    Ok(data.get(key)
        .ok_or_else(|| format!("{} is mandatory", key))?
        .join(" "))
}

pub fn mandatory_whitespace_list(
    data: &HashMap<&str, Vec<&str>>,
    key: &str,
) -> Result<Vec<String>> {
    Ok(mandatory_single_line(data, key)?
        .split_whitespace()
        .map(|x| x.to_string())
        .collect())
}

pub fn one_line<'a>(lines: &[&'a str]) -> Result<&'a str> {
    ensure!(1 == lines.len(), "{:?} isn't exactly one line", lines);
    Ok(lines[0])
}

pub fn joined(lines: &[&str]) -> String {
    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use super::Line;
    use super::scan;
    use errors::*;

    #[test]
    fn single_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar"])],
            scan("Foo: bar\n").collect::<Result<Vec<Line>>>().unwrap()
        );
    }

    #[test]
    fn multi_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz"])],
            scan("Foo:\n bar\n baz\n")
                .collect::<Result<Vec<Line>>>()
                .unwrap()
        );
    }

    #[test]
    fn multi_line_joined() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz", "quux"])],
            scan("Foo: bar\n baz\n quux\n")
                .collect::<Result<Vec<Line>>>()
                .unwrap()
        );
    }

    #[test]
    fn walkies() {
        use super::Section;
        use errors::*;
        use std::io;

        let parts: Result<Vec<Vec<u8>>> =
            Section::new(io::Cursor::new(b"foo\nbar\n\nbaz\n")).collect();
        assert_eq!(
            vec![b"foo\nbar\n".to_vec(), b"baz\n".to_vec()],
            parts.unwrap()
        );
    }
}
