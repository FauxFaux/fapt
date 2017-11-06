use errors::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Entry<'s> {
    src: bool,
    url: &'s str,
    dist: &'s str,
    components: Vec<&'s str>,
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
    dist: word >>
    components: many1!(preceded!(spaces, word)) >>
    ( Entry { src, url, dist, components })
));

pub fn read(from: &str) -> Result<Vec<Entry>> {
    let mut ret = Vec::with_capacity(10);

    for (computer_no, line) in from.lines().enumerate() {
        let no = computer_no + 1;

        let line = match line.find('#') {
            Some(comment) => &line[..comment],
            None => line,
        }.trim();

        if line.is_empty() {
            continue;
        }

        use nom::IResult::*;
        match single_line(line) {
            Done("", en) => ret.push(en),
            Done(trailing, _) => bail!("trailing garbage on line {}: {:?}", no, trailing),
            other => bail!("parse error on line {}: {:?}", no, other),
        }
    }

    Ok(ret)
}

//pub fn load<P: AsRef<Path>>(path: P) -> Vec<Entry> {}

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
                    url: "http://foo",
                    dist: "bar",
                    components: vec!["bar", "quux"],
                },
                Entry {
                    src: true,
                    url: "http://foo",
                    dist: "bar",
                    components: vec!["bar", "quux"],
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
