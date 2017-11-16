use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::BufRead;

use mailparse::dateparse;

use errors::*;

pub fn scan(block: &str) -> Result<Vec<(&str, Vec<&str>)>> {
    let mut it = block.lines().peekable();
    let mut ret = Vec::new();
    loop {
        let line = match it.next() {
            Some(line) => line,
            None => break,
        };

        let colon = line.find(':')
            .ok_or_else(|| format!("expected a key: in {:?}", line))?;
        let (key, first_val) = line.split_at(colon);
        let first_val = first_val[1..].trim();
        let mut sub = Vec::new();
        if !first_val.is_empty() {
            sub.push(first_val);
        }

        loop {
            match it.peek() {
                Some(line) if line.starts_with(' ') => {
                    sub.push(line.trim());
                }
                Some(_) | None => break,
            }

            it.next().expect("just peeked");
        }

        ret.push((key, sub));
    }

    Ok(ret)
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

#[cfg(test)]
mod tests {
    use super::scan;

    #[test]
    fn single_line_header() {
        assert_eq!(vec![("Foo", vec!["bar"])], scan("Foo: bar\n").unwrap());
    }

    #[test]
    fn multi_line_header() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz"])],
            scan("Foo:\n bar\n baz\n").unwrap()
        );
    }

    #[test]
    fn multi_line_joined() {
        assert_eq!(
            vec![("Foo", vec!["bar", "baz", "quux"])],
            scan("Foo: bar\n baz\n quux\n").unwrap()
        );
    }

    #[test]
    fn walkies() {
        use std::io;
        use super::Section;
        use errors::*;

        let parts: Result<Vec<Vec<u8>>> =
            Section::new(io::Cursor::new(b"foo\nbar\n\nbaz\n")).collect();
        assert_eq!(
            vec![b"foo\nbar\n".to_vec(), b"baz\n".to_vec()],
            parts.unwrap()
        );
    }
}
