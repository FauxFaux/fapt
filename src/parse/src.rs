use std::collections::HashMap;

use failure::bail;
use failure::ensure;
use failure::Error;

use super::types::SourceFormat;

fn parse_format(string: &str) -> Result<SourceFormat, Error> {
    Ok(match string {
        "3.0 (quilt)" => SourceFormat::Quilt3dot0,
        "1.0" => SourceFormat::Original,
        "3.0 (git)" => SourceFormat::Git3dot0,
        "3.0 (native)" => SourceFormat::Native3dot0,
        other => bail!("unsupported source format: '{}'", other),
    })
}

fn take_checksums<'a>(
    map: &mut HashMap<&str, &'a str>,
    key: &str,
) -> Result<Option<HashMap<(&'a str, u64), &'a str>>, Error> {
    Ok(match map.remove(key) {
        Some(s) => Some(parse_checksums(s)?),
        None => None,
    })
}

fn parse_checksums(from: &str) -> Result<HashMap<(&str, u64), &str>, Error> {
    let mut ret = HashMap::new();
    for line in from.lines() {
        let parts: Vec<&str> = line.trim().split(' ').collect();
        ensure!(3 == parts.len(), "invalid checksums line: {:?}", line);
        ret.insert((parts[2], parts[1].parse()?), parts[0]);
    }

    Ok(ret)
}
