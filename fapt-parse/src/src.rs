use std::collections::HashMap;

use errors::*;
use fields;
use types::Source;
use types::SourceFormat;
use vcs;

use as_u32;
#[cfg(capnp)]
use fill_identity;
use parse_priority;
#[cfg(capnp)]
use fill_dep;

#[cfg(capnp)]
pub fn populate<'a>(
    mut output: source::Builder,
    map: &mut HashMap<&'a str, &str>,
) -> Result<(Vec<&'a str>, Vec<String>)> {
    if let Some(format) = map.remove("Format") {
        output.set_format(parse_format(format)?);
    }

    if let Some(list) = map.get("Package-List") {
        let lines: Vec<&str> = list.split('\n').map(|x| x.trim()).collect();
        let mut builder = output.borrow().init_binaries(as_u32(lines.len()));
        for (i, line) in lines.into_iter().enumerate() {
            let mut builder = builder.borrow().get(as_u32(i));
            let parts: Vec<&str> = line.split(' ').collect();
            builder.set_name(parts[0]);
            builder.set_style(parts[1]);
            builder.set_section(parts[2]);
            builder
                .set_priority(parse_priority(parts[3]).chain_err(|| "priority inside package list")?);

            if parts.len() > 4 {
                let mut builder = builder.init_extras(as_u32(parts.len() - 4));
                for (i, part) in parts[4..].iter().enumerate() {
                    builder.set(as_u32(i), part);
                }
            }
        }
    }

    if let Some(md5) = take_checksums(map, "Files")? {
        let sha1 = take_checksums(map, "Checksums-Sha1")?;
        let sha256 = take_checksums(map, "Checksums-Sha256")?;
        let sha512 = take_checksums(map, "Checksums-Sha512")?;

        let keys = {
            let mut keys: Vec<&(&str, u64)> = md5.keys().collect();
            keys.sort();
            keys
        };

        let mut builder = output.borrow().init_files(as_u32(keys.len()));
        for (i, key) in keys.into_iter().enumerate() {
            let (name, size) = *key;
            let mut builder = builder.borrow().get(as_u32(i));
            builder.set_name(name);
            builder.set_size(size);
            builder.set_md5(md5[key]);
            if let Some(c) = sha1.as_ref().and_then(|m| m.get(key)) {
                builder.set_sha1(c);
            }

            if let Some(c) = sha256.as_ref().and_then(|m| m.get(key)) {
                builder.set_sha256(c);
            }

            if let Some(c) = sha512.as_ref().and_then(|m| m.get(key)) {
                builder.set_sha512(c);
            }
        }
    }

    vcs::extract(map, &mut output.borrow())?;

    fill_dep(map, "Build-Depends", |len| {
        output.borrow().init_build_dep(len)
    })?;

    fill_dep(map, "Build-Depends-Arch", |len| {
        output.borrow().init_build_dep_arch(len)
    })?;

    fill_dep(map, "Build-Depends-Indep", |len| {
        output.borrow().init_build_dep_indep(len)
    })?;

    fill_dep(map, "Build-Conflicts", |len| {
        output.borrow().init_build_conflict(len)
    })?;

    fill_dep(map, "Build-Conflicts-Arch", |len| {
        output.borrow().init_build_conflict_arch(len)
    })?;

    fill_dep(map, "Build-Conflicts-Indep", |len| {
        output.borrow().init_build_conflict_indep(len)
    })?;

    fill_identity(map.remove("Uploaders"), |len| {
        output.borrow().init_uploaders(len)
    })?;

    let mut unparsed = output.init_unparsed();

    let mut unrecognised_fields = Vec::new();
    for (key, val) in map {
        if !fields::set_field_source(key, val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?
        {
            unrecognised_fields.push(*key);
        }
    }

    Ok((unrecognised_fields, Vec::new()))
}

fn parse_format(string: &str) -> Result<SourceFormat> {
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
) -> Result<Option<HashMap<(&'a str, u64), &'a str>>> {
    Ok(match map.remove(key) {
        Some(s) => Some(parse_checksums(s)?),
        None => None,
    })
}

fn parse_checksums(from: &str) -> Result<HashMap<(&str, u64), &str>> {
    let mut ret = HashMap::new();
    for line in from.lines() {
        let parts: Vec<&str> = line.trim().split(' ').collect();
        ensure!(3 == parts.len(), "invalid checksums line: {:?}", line);
        ret.insert((parts[2], parts[1].parse()?), parts[0]);
    }

    Ok(ret)
}
