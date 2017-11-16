extern crate capnp;
#[macro_use]
extern crate error_chain;

extern crate md5;

#[macro_use]
extern crate nom;

use std::collections::HashMap;

use capnp::serialize;

mod apt_capnp;
mod bin;
mod deps;
mod errors;
mod fields;
mod ident;
mod src;
mod vcs;

use apt_capnp::item;
use apt_capnp::entry;
use apt_capnp::package;

use apt_capnp::RawPackageType;
use apt_capnp::Priority;

use apt_capnp::dependency;
use apt_capnp::single_dependency;
use apt_capnp::identity;

use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let stdin = ::std::io::stdin();
    let mut stdin = stdin.lock();

    let stdout = ::std::io::stdout();
    let mut stdout = stdout.lock();

    loop {
        let input = serialize::read_message(&mut stdin, capnp::message::ReaderOptions::new())?;

        let input = input.get_root::<item::Reader>()?;
        let mut message = capnp::message::Builder::new_default();

        match input.which()? {
            item::End(()) => return Ok(()),
            item::Package(_) => bail!("unexpected item type in stream: already processed?"),
            item::Index(index) => {
                message.init_root::<item::Builder>().set_index(index?)?;
            }
            item::Raw(input) => {
                let input = input?;

                let mut package = message.init_root::<item::Builder>().init_package();

                let mut map = to_map(input.get_entries()?)?;

                let name = if let Some(name) = map.remove("Package") {
                    package.set_name(name);
                    name
                } else {
                    "[not available]"
                };

                let version = if let Some(version) = map.remove("Version") {
                    package.set_version(version);
                    version
                } else {
                    "[not available]"
                };

                let initial_errors = fill_package(&mut package, &mut map).chain_err(|| {
                    format!("filling basic information for {:?} {:?}", name, version)
                })?;

                let (unrecognised, mut errors) = {
                    let style = package.borrow().init_style();

                    match input.get_type()? {
                        RawPackageType::Source => src::populate(style.init_source(), &mut map),
                        RawPackageType::Binary => bin::populate(style.init_binary(), &mut map),
                    }.chain_err(|| format!("parsing package {:?} {:?}", name, version))?
                };

                errors.extend(initial_errors);

                if !errors.is_empty() {
                    let mut builder = package.borrow().init_parse_errors(as_u32(errors.len()));
                    for (i, field) in errors.into_iter().enumerate() {
                        builder.set(as_u32(i), &field);
                    }
                }

                if !unrecognised.is_empty() {
                    let mut builder = package
                        .borrow()
                        .init_unrecognised_fields(as_u32(unrecognised.len()));
                    for (i, field) in unrecognised.into_iter().enumerate() {
                        builder.set(as_u32(i), field);
                    }
                }
            }
        };

        serialize::write_message(&mut stdout, &message)?;
    }
}

fn fill_package(
    output: &mut package::Builder,
    map: &mut HashMap<&str, &str>,
) -> Result<Vec<String>> {
    let mut allowed_parse_errors = Vec::new();

    if let Some(priority) = map.remove("Priority") {
        output.set_priority(parse_priority(priority).chain_err(|| "top-level priority")?);
    }

    if let Some(arch) = map.remove("Architecture") {
        let mut parts: Vec<&str> = arch.split(' ').map(|x| x.trim()).collect();
        parts.sort();

        let mut builder = output.borrow().init_arch(as_u32(parts.len()));
        for (i, part) in parts.into_iter().enumerate() {
            builder.set(as_u32(i), part);
        }
    }

    if let Err(e) = fill_identity(map.remove("Maintainer"), |len| {
        output.borrow().init_maintainer(len)
    }) {
        allowed_parse_errors.push(format!("Couldn't parse Maintainer: {:?}", e))
    }

    if let Err(e) = fill_identity(map.remove("Original-Maintainer"), |len| {
        output.borrow().init_original_maintainer(len)
    }) {
        allowed_parse_errors.push(format!("Couldn't parse Original-Maintainer: {:?}", e));
    }

    Ok(allowed_parse_errors)
}

fn to_map<'a>(reader: capnp::struct_list::Reader<entry::Owned>) -> Result<HashMap<&str, &str>> {
    let mut ret = HashMap::with_capacity(reader.len() as usize);

    for i in 0..reader.len() {
        let reader = reader.get(i);
        ret.insert(reader.get_key()?, reader.get_value()?);
    }

    Ok(ret)
}

fn fill_identity<'a, F>(value: Option<&str>, into: F) -> Result<()>
where
    F: FnOnce(u32)
        -> capnp::struct_list::Builder<'a, identity::Owned>,
{
    if value.is_none() {
        return Ok(());
    }

    let idents = ident::read(value.unwrap())?;

    let mut builder = into(as_u32(idents.len()));

    for (i, ident) in idents.into_iter().enumerate() {
        let mut builder = builder.borrow().get(as_u32(i));
        if !ident.name.is_empty() {
            builder.set_name(&ident.name);
        }

        if !ident.email.is_empty() {
            builder.set_email(&ident.email);
        }
    }

    Ok(())
}

fn parse_priority(string: &str) -> Result<Priority> {
    Ok(match string {
        "required" => Priority::Required,
        "important" => Priority::Important,
        "standard" => Priority::Standard,
        "optional" => Priority::Optional,
        "extra" => Priority::Extra,
        "source" => Priority::Source,
        "unknown" => Priority::Unknown,
        other => bail!("unsupported priority: '{}'", other),
    })
}

fn fill_dep<'a, F>(map: &mut HashMap<&str, &str>, key: &str, init: F) -> Result<()>
where
    F: FnOnce(u32)
        -> capnp::struct_list::Builder<'a, dependency::Owned>,
{
    match map.remove(key) {
        Some(raw) => fill_dep_in(raw, init).chain_err(|| format!("parsing {}", key)),
        None => Ok(()),
    }
}

fn fill_dep_in<'a, F>(raw: &str, init: F) -> Result<()>
where
    F: FnOnce(u32)
        -> capnp::struct_list::Builder<'a, dependency::Owned>,
{
    let read = deps::read(raw)?;

    if read.is_empty() {
        return Ok(());
    }

    let mut builder = init(as_u32(read.len()));
    for (i, alt) in read.into_iter().enumerate() {
        let mut builder = builder
            .borrow()
            .get(as_u32(i))
            .init_alternate(as_u32(alt.alternate.len()));
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
        let mut builder = builder
            .borrow()
            .init_version_constraints(as_u32(single.version_constraints.len()));
        for (i, version) in single.version_constraints.into_iter().enumerate() {
            let mut builder = builder.borrow().get(as_u32(i));
            builder.set_version(&version.version);
            use deps::Op;
            use apt_capnp::ConstraintOperator::*;
            builder.set_operator(match version.operator {
                Op::Ge => Ge,
                Op::Eq => Eq,
                Op::Le => Le,
                Op::Gt => Gt,
                Op::Lt => Lt,
            });
        }
    }

    if !single.arch_filter.is_empty() {
        let mut builder = builder
            .borrow()
            .init_arch_filter(as_u32(single.arch_filter.len()));
        for (i, arch) in single.arch_filter.into_iter().enumerate() {
            builder.set(as_u32(i), &arch);
        }
    }

    if !single.stage_filter.is_empty() {
        let mut builder = builder
            .borrow()
            .init_stage_filter(as_u32(single.stage_filter.len()));
        for (i, stage) in single.stage_filter.into_iter().enumerate() {
            builder.set(as_u32(i), &stage);
        }
    }
}

fn yes_no(value: &str) -> Result<bool> {
    match value {
        "yes" => Ok(true),
        "no" => Ok(false),
        other => bail!("invalid value for yes/no: {:?}", other),
    }
}

fn blank_to_null<F>(value: &str, into: F)
where
    F: FnOnce(&str),
{
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return;
    }

    into(cleaned)
}

fn as_u32(val: usize) -> u32 {
    assert!(
        val <= (std::u32::MAX as usize),
        "can't have more than 2^32 anything"
    );
    val as u32
}
