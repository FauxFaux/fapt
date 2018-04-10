#[macro_use]
extern crate error_chain;

extern crate md5;

#[macro_use]
extern crate nom;

use std::collections::HashMap;

mod bin;
pub mod deps;
mod errors;
mod ident;
mod src;
mod types;
mod vcs;

use types::Entry;
use types::Item;
use types::Package;

use types::Priority;
use types::RawPackageType;

use types::Dependency;
use types::Identity;
use types::SingleDependency;

pub use errors::*;

#[cfg(capnp)]
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

#[cfg(capnp)]
fn to_map<'a>(reader: capnp::struct_list::Reader<entry::Owned>) -> Result<HashMap<&str, &str>> {
    let mut ret = HashMap::with_capacity(reader.len() as usize);

    for i in 0..reader.len() {
        let reader = reader.get(i);
        ret.insert(reader.get_key()?, reader.get_value()?);
    }

    Ok(ret)
}

#[cfg(capnp)]
fn fill_identity<'a, F>(value: Option<&str>, into: F) -> Result<()>
where
    F: FnOnce(u32) -> capnp::struct_list::Builder<'a, identity::Owned>,
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

#[cfg(capnp)]
fn fill_dep<'a, F>(map: &mut HashMap<&str, &str>, key: &str, init: F) -> Result<()>
where
    F: FnOnce(u32) -> capnp::struct_list::Builder<'a, dependency::Owned>,
{
    match map.remove(key) {
        Some(raw) => fill_dep_in(raw, init).chain_err(|| format!("parsing {}", key)),
        None => Ok(()),
    }
}

#[cfg(capnp)]
fn fill_dep_in<'a, F>(raw: &str, init: F) -> Result<()>
where
    F: FnOnce(u32) -> capnp::struct_list::Builder<'a, dependency::Owned>,
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

#[cfg(capnp)]
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
            use types::ConstraintOperator::*;
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
