//! Load `Entry` objects from from a _classic_ sources list. (e.g. `/etc/*apt/sources.list`).

use std::io::BufRead;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Error;

use pyo3::basic::CompareOp;
use pyo3::prelude::{pyclass, pymethods, IntoPy, Py, PyAny, PyModule, PyResult, Python};

/// Our representation of a classic sources list entry.
#[derive(Debug, PartialEq, Eq, Clone)]
#[pyclass]
pub struct Entry {
    pub src: bool,
    pub url: String,
    pub suite_codename: String,
    pub components: Vec<String>,
    pub arch: Option<String>,
}

#[pymethods]
impl Entry {
    #[new]
    fn py_new(
        src: bool,
        url: String,
        suite_codename: String,
        components: Vec<String>,
        arch: Option<String>,
    ) -> Self {
        Entry {
            src,
            url,
            suite_codename,
            components,
            arch,
        }
    }

    fn __richcmp__(&self, py: Python<'_>, other: &Self, op: CompareOp) -> Py<PyAny> {
        match op {
            CompareOp::Eq => (self == other).into_py(py),
            CompareOp::Ne => (self != other).into_py(py),
            _ => py.NotImplemented(),
        }
    }
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
    let arch = match parts.peek() {
        Some(&val) if val.starts_with("[") => {
            parts.next();
            Some(val)
        }
        Some(_) => None,
        None => bail!("unexpected end of line looking for arch or url"),
    };

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
            arch: arch.map(|arch| arch.to_string()),
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
                },
                Entry {
                    src: true,
                    arch: None,
                    url: "http://foo/".to_string(),
                    suite_codename: "bar".to_string(),
                    components: vec!["baz".to_string(), "quux".to_string()],
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
}

pub fn py_sources_list(py: Python<'_>) -> PyResult<&PyModule> {
    let mut m = PyModule::new(py, "sources_list")?;
    m.add_class::<Entry>()?;
    Ok(m)
}
