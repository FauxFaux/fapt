use errors::*;

use std::fs;
use std::io;
use std::path;

use std::io::BufRead;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub src: bool,
    pub url: String,
    pub suite_codename: String,
    pub components: Vec<String>,
}

fn line_space(c: char) -> bool {
    ' ' == c || '\t' == c
}

fn dist_component_char(c: char) -> bool {
    c.is_alphanumeric() || '-' == c
}

named!(deb_or_src<&str, bool>, alt!(
    tag!("deb-src") => { |_| true } |
    tag!("deb") => { |_| false }
));

named!(url<&str, &str>, take_till1_s!(line_space));
named!(word<&str, &str>, take_while1_s!(dist_component_char));
named!(spaces<&str, &str>, take_while1_s!(line_space));

named!(single_line<&str, Entry>, do_parse!(
    src: deb_or_src >>
    spaces >>
    url: url >>
    spaces >>
    suite: word >>
    components: many1!(preceded!(spaces, word)) >>
    ( Entry {
        src,
        url: if url.ends_with('/') { url.to_string() } else { format!("{}/", url) },
        suite_codename: suite.to_string(),
        components: components.into_iter().map(|x| x.to_string()).collect()
     } )
));

fn read_single_line(line: &str) -> Option<Result<Entry>> {
    let line = match line.find('#') {
        Some(comment) => &line[..comment],
        None => line,
    }.trim();

    if line.is_empty() {
        return None;
    }

    use nom::IResult::*;
    Some(match single_line(line) {
        Done("", en) => Ok(en),
        Done(trailing, _) => Err(format!("trailing garbage: {:?}", trailing).into()),
        other => Err(format!("other error: {:?}", other).into()),
    })
}

fn read_single_line_number(line: &str, no: usize) -> Option<Result<Entry>> {
    read_single_line(line).map(|r| r.chain_err(|| format!("parsing line {}", no + 1)))
}

pub fn read(from: &str) -> Result<Vec<Entry>> {
    from.lines()
        .enumerate()
        .flat_map(|(no, line)| read_single_line_number(line, no))
        .collect()
}

pub fn load<P: AsRef<path::Path>>(path: P) -> Result<Vec<Entry>> {
    io::BufReader::new(fs::File::open(path)?)
        .lines()
        .enumerate()
        .flat_map(|(no, line)| match line {
            Ok(line) => read_single_line_number(&line, no),
            Err(e) => Some(Err(
                Error::with_chain(e, format!("reading around line {}", no)),
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::read;
    use super::Entry;

    #[test]
    fn simple() {
        assert_eq!(
            vec![
                Entry {
                    src: false,
                    url: "http://foo/".to_string(),
                    suite_codename: "bar".to_string(),
                    components: vec!["baz".to_string(), "quux".to_string()],
                },
                Entry {
                    src: true,
                    url: "http://foo/".to_string(),
                    suite_codename: "bar".to_string(),
                    components: vec!["baz".to_string(), "quux".to_string()],
                },
            ],

            read(
                r"
deb     http://foo  bar  baz quux
deb-src http://foo  bar  baz quux
",
            ).unwrap()
        );
    }
}
