use capnp;

use std::collections::HashMap;

use apt_capnp::dependency;
use apt_capnp::single_dependency;
use apt_capnp::source;
use apt_capnp::SourceFormat;

use deps;
use errors::*;
use fields;
use vcs;

use as_u32;
use fill_identity;
use fill_priority;

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

    fill_build_dep(map.get("Build-Depends"), |len| {
        output.borrow().init_build_dep(len)
    }).chain_err(|| "parsing Build-Depends")?;

    fill_build_dep(map.get("Build-Depends-Arch"), |len| {
        output.borrow().init_build_dep_arch(len)
    }).chain_err(|| "parsing Build-Depends-Arch")?;

    fill_build_dep(map.get("Build-Depends-Indep"), |len| {
        output.borrow().init_build_dep_indep(len)
    }).chain_err(|| "parsing Build-Depends-Indep")?;

    fill_build_dep(map.get("Build-Conflicts"), |len| {
        output.borrow().init_build_conflict(len)
    }).chain_err(|| "parsing Build-Conflicts")?;

    fill_build_dep(map.get("Build-Conflicts-Arch"), |len| {
        output.borrow().init_build_conflict_arch(len)
    }).chain_err(|| "parsing Build-Conflicts-Arch")?;

    fill_build_dep(map.get("Build-Conflicts-Indep"), |len| {
        output.borrow().init_build_conflict_indep(len)
    }).chain_err(|| "parsing Build-Conflicts-Indep")?;

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

fn fill_build_dep<'a, F>(raw: Option<&&str>, init: F) -> Result<()>
where
    F: FnOnce(u32) -> capnp::struct_list::Builder<'a, dependency::Owned>,
{
    if raw.is_none() {
        return Ok(());
    }

    let read = deps::read(raw.unwrap())?;

    if read.is_empty() {
        return Ok(());
    }

    let mut builder = init(as_u32(read.len()));
    for (i, alt) in read.into_iter().enumerate() {
        let mut builder = builder.borrow().get(as_u32(i)).init_alternate(
            as_u32(alt.alternate.len()),
        );
        for (i, single) in alt.alternate.into_iter().enumerate() {
            let builder = builder.borrow().get(as_u32(i));
            fill_single_dep(single, builder);
        }
    }

    Ok(())
}


fn fill_single_dep(single: deps::SingleDep, mut builder: single_dependency::Builder) {
    builder.set_package(&single.package);

    if let Some(ref arch) = single.arch {
        builder.set_arch(arch);
    }

    if !single.version_constraints.is_empty() {
        let mut builder = builder.borrow().init_version_constraints(
            as_u32(single.version_constraints.len()),
        );
        for (i, version) in single.version_constraints.into_iter().enumerate() {
            let mut builder = builder.borrow().get(as_u32(i));
            builder.set_version(&version.version);
            use deps::Op;
            match version.operator {
                Op::Ge => builder.init_operator().set_ge(()),
                Op::Eq => builder.init_operator().set_eq(()),
                Op::Le => builder.init_operator().set_le(()),
                Op::Gt => builder.init_operator().set_gt(()),
                Op::Lt => builder.init_operator().set_lt(()),
            }
        }
    }

    if !single.arch_filter.is_empty() {
        let mut builder = builder.borrow().init_arch_filter(
            as_u32(single.arch_filter.len()),
        );
        for (i, arch) in single.arch_filter.into_iter().enumerate() {
            builder.set(as_u32(i), &arch);
        }
    }

    if !single.stage_filter.is_empty() {
        let mut builder = builder.borrow().init_stage_filter(
            as_u32(single.stage_filter.len()),
        );
        for (i, stage) in single.stage_filter.into_iter().enumerate() {
            builder.set(as_u32(i), &stage);
        }
    }
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
