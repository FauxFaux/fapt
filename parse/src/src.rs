use std::collections::HashMap;

use apt_capnp::source;
use apt_capnp::SourceFormat;

use errors::*;
use fields;
use vcs;

use as_u32;
use fill_identity;
use fill_priority;
use fill_dep;

pub fn populate(mut output: source::Builder, map: HashMap<&str, &str>) -> Result<()> {

    output.set_format(parse_format(&map["Format"])?);

    if let Some(list) = map.get("Package-List") {
        let lines: Vec<&str> = list.split('\n').map(|x| x.trim()).collect();
        let mut builder = output.borrow().init_binaries(as_u32(lines.len()));
        for (i, line) in lines.into_iter().enumerate() {
            let mut builder = builder.borrow().get(as_u32(i));
            let parts: Vec<&str> = line.split(' ').collect();
            builder.set_name(parts[0]);
            builder.set_style(parts[1]);
            builder.set_section(parts[2]);
            fill_priority(builder.borrow().init_priority(), parts[3])
                .chain_err(|| "priority inside package list")?;

            if parts.len() > 4 {
                let mut builder = builder.init_extras(as_u32(parts.len() - 4));
                for (i, part) in parts[4..].iter().enumerate() {
                    builder.set(as_u32(i), part);
                }
            }
        }
    }

    #[cfg(todo)]
    {
        let reader = input.get_files()?;
        let mut builder = output.borrow().init_files(reader.len());
        for i in 0..reader.len() {
            let reader = reader.borrow().get(i);
            let mut builder = builder.borrow().get(i);
            blank_to_null(reader.get_name()?, |x| builder.set_name(x));
            builder.set_size(reader.get_size());
            blank_to_null(reader.get_md5()?, |x| builder.set_md5(x));
            blank_to_null(reader.get_sha1()?, |x| builder.set_sha1(x));
            blank_to_null(reader.get_sha256()?, |x| builder.set_sha256(x));
            blank_to_null(reader.get_sha512()?, |x| builder.set_sha512(x));
        }
    }

    vcs::extract(&map, &mut output.borrow())?;

    fill_dep(
        &map,
        "Build-Depends",
        |len| output.borrow().init_build_dep(len),
    )?;

    fill_dep(&map, "Build-Depends-Arch", |len| {
        output.borrow().init_build_dep_arch(len)
    })?;

    fill_dep(&map, "Build-Depends-Indep", |len| {
        output.borrow().init_build_dep_indep(len)
    })?;

    fill_dep(&map, "Build-Conflicts", |len| {
        output.borrow().init_build_conflict(len)
    })?;

    fill_dep(&map, "Build-Conflicts-Arch", |len| {
        output.borrow().init_build_conflict_arch(len)
    })?;

    fill_dep(&map, "Build-Conflicts-Indep", |len| {
        output.borrow().init_build_conflict_indep(len)
    })?;

    fill_identity(map.get("Uploaders"), |len| {
        output.borrow().init_uploaders(len)
    })?;

    let mut unparsed = output.init_unparsed();

    for (key, val) in map.into_iter() {
        if fields::HANDLED_FIELDS_SOURCE.contains(&key) {
            continue;
        }

        fields::set_field_source(&key, &val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?;
    }

    Ok(())
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
