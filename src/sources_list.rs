//! Load `Entry` objects from from a _classic_ sources list. (e.g. `/etc/*apt/sources.list`).

use std::io::BufRead;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;

/// Our representation of a classic sources list entry.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Entry {
    pub src: bool,
    pub url: String,
    pub suite_codename: String,
    pub components: Vec<String>,
    pub arch: Option<String>,
    pub untrusted: bool,
}

#[derive(Default)]
struct ParsedOpts {
    arch: Option<String>,
    untrusted: Option<bool>,
}

fn parse_opts(opt_parts: Vec<&str>) -> Result<ParsedOpts, Error> {
    let mut ret = ParsedOpts::default();
    for opt in opt_parts {
        let parts: Vec<_> = opt.split("=").collect();
        match parts.len() {
            2 => match parts[0] {
                "arch" => ret.arch = Some(parts[1].to_string()),
                "untrusted" => ret.untrusted = Some(parts[1] == "yes"),
                other => bail!("unknown option: {}", other),
            },
            _ => bail!("multiple = in option"),
        }
    }
    Ok(ret)
}

fn read_single_line(line: &str) -> Result<Vec<Entry>, Error> {
    let line = match line.find('#') {
        Some(comment) => &line[..comment],
        None => line,
    }
    .trim();

    if line.is_empty() {
        return Ok(Vec::new());
    }

    let mut parts = line.split_whitespace().peekable();

    let src = parts
        .next()
        .ok_or_else(|| anyhow!("deb{{,s,-src}} section required"))?;

    let mut opt_parts = vec![];
    if let Some(&(mut val)) = parts.peek() {
        if val.starts_with("[") {
            val = val.strip_prefix("[").expect("opening [");
            parts.next();
            loop {
                if val.ends_with("]") {
                    opt_parts.push(val.strip_suffix("]").expect("closing ]"));
                    break;
                }
                opt_parts.push(val);
                match parts.next() {
                    Some(next) => {
                        val = next;
                    }
                    None => bail!("unexpected end of line reading options"),
                }
            }
        }
    }

    let url = parts
        .next()
        .ok_or_else(|| anyhow!("url section required"))?;
    let suite = parts
        .next()
        .ok_or_else(|| anyhow!("suite section required"))?;

    let components: Vec<&str> = parts.collect();

    let srcs: &[bool] = match src {
        "deb" => &[false],
        "deb-src" => &[true],
        "debs" => &[false, true],
        other => bail!("unsupported deb-src tag: {:?}", other),
    };

    let mut ret = Vec::with_capacity(srcs.len());

    let parsed_opts = parse_opts(opt_parts)?;
    for src in srcs {
        ret.push(Entry {
            src: *src,
            url: if url.ends_with('/') {
                url.to_string()
            } else {
                format!("{}/", url)
            },
            suite_codename: suite.to_string(),
            components: components.iter().map(|x| x.to_string()).collect(),
            arch: parsed_opts.arch.clone(),
            untrusted: parsed_opts.untrusted.unwrap_or(false),
        });
    }

    Ok(ret)
}

fn read_single_line_number(line: &str, no: usize) -> Result<Vec<Entry>, Error> {
    Ok(read_single_line(line).with_context(|| anyhow!("parsing line {}", no + 1))?)
}

/// Read `Entry` objects from some `sources.list` lines.
pub fn read<R: BufRead>(from: R) -> Result<Vec<Entry>, Error> {
    Ok(from
        .lines()
        .enumerate()
        .map(|(no, line)| match line {
            Ok(line) => read_single_line_number(&line, no),
            Err(e) => Err(anyhow!("reading around line {}: {:?}", no, e)),
        })
        .collect::<Result<Vec<Vec<Entry>>, Error>>()?
        .into_iter()
        .flat_map(|x| x)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::read;
    use super::Entry;

    #[test]
    fn simple() {
        assert_eq!(
            vec![
                Entry {
                    src: false,
                    arch: None,
                    url: "http://foo/".to_string(),
                    suite_codename: "bar".to_string(),
                    components: vec!["baz".to_string(), "quux".to_string()],
                    untrusted: false,
                },
                Entry {
                    src: true,
                    arch: None,
                    url: "http://foo/".to_string(),
                    suite_codename: "bar".to_string(),
                    components: vec!["baz".to_string(), "quux".to_string()],
                    untrusted: false,
                },
            ],
            read(io::Cursor::new(
                r"
deb     http://foo  bar  baz quux
deb-src http://foo  bar  baz quux
",
            ))
            .unwrap()
        );
    }

    #[test]
    fn arch() {
        assert_eq!(
            vec![Entry {
                src: false,
                arch: Some("amd64".to_string()),
                url: "http://foo/".to_string(),
                suite_codename: "bar".to_string(),
                components: vec!["baz".to_string(), "quux".to_string()],
                untrusted: false,
            },],
            read(io::Cursor::new(
                r"
deb [arch=amd64] http://foo  bar  baz quux
",
            ))
            .unwrap()
        );
    }

    #[test]
    fn untrusted() {
        assert_eq!(
            vec![Entry {
                src: false,
                arch: None,
                url: "http://foo/".to_string(),
                suite_codename: "bar".to_string(),
                components: vec!["baz".to_string(), "quux".to_string()],
                untrusted: true,
            },],
            read(io::Cursor::new(
                r"
deb [untrusted=yes] http://foo  bar  baz quux
",
            ))
            .unwrap()
        );
    }

    #[test]
    fn untrusted_not_yes() {
        assert_eq!(
            vec![Entry {
                src: false,
                arch: None,
                url: "http://foo/".to_string(),
                suite_codename: "bar".to_string(),
                components: vec!["baz".to_string(), "quux".to_string()],
                untrusted: false,
            },],
            read(io::Cursor::new(
                r"
deb [untrusted=yeppers] http://foo  bar  baz quux
",
            ))
            .unwrap()
        );
    }

    #[test]
    fn multiple_opt() {
        assert_eq!(
            vec![Entry {
                src: false,
                arch: Some("amd64".to_string()),
                url: "http://foo/".to_string(),
                suite_codename: "bar".to_string(),
                components: vec!["baz".to_string(), "quux".to_string()],
                untrusted: true,
            },],
            read(io::Cursor::new(
                r"
deb [untrusted=yes arch=amd64] http://foo  bar  baz quux
",
            ))
            .unwrap()
        );
    }
}
