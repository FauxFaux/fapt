extern crate capnp;
#[macro_use]
extern crate error_chain;

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
use apt_capnp::priority;

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

        {
            let mut package = message.init_root::<item::Builder>().init_package();

            match input.which()? {
                item::End(()) => return Ok(()),
                item::Package(_) => bail!("unexpected item type in stream: already processed?"),
                item::Index(_) => unimplemented!(),
                item::Raw(input) => {
                    let input = input?;

                    let map = to_map(input.get_entries()?)?;

                    fill_package(&mut package, &map)?;

                    let style = package.init_style();

                    match input.get_type()? {
                        RawPackageType::Source => src::populate(style.init_source(), map)?,
                        RawPackageType::Binary => bin::populate(style.init_binary(), map)?,
                    }
                }
            };
        }

        serialize::write_message(&mut stdout, &message)?;
    }
}

fn fill_package<'a>(output: &mut package::Builder, map: &HashMap<String, String>) -> Result<()> {
    if let Some(name) = map.get("Package") {
        output.set_name(name);
    }

    if let Some(version) = map.get("Version") {
        output.set_version(version);
    }

    if let Some(priority) = map.get("Priority") {
        fill_priority(output.borrow().init_priority(), priority)
            .chain_err(|| "top-level priority")?;
    }

    {
        let mut parts: Vec<&str> = map["Architecture"].split(' ').map(|x| x.trim()).collect();
        parts.sort();

        let mut builder = output.borrow().init_arch(as_u32(parts.len()));
        for (i, part) in parts.into_iter().enumerate() {
            builder.set(as_u32(i), part);
        }
    }

    fill_identity(map.get("Maintainer"), |len| {
        output.borrow().init_maintainer(len)
    }).chain_err(|| "parsing Maintainer")?;

    fill_identity(map.get("Original-Maintainer"), |len| {
        output.borrow().init_original_maintainer(len)
    }).chain_err(|| "parsing Original-Maintainer")?;

    Ok(())
}

fn to_map(reader: capnp::struct_list::Reader<entry::Owned>) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(reader.len() as usize);

    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;

        ret.insert(key.to_string(), reader.get_value()?.to_string());
    }

    Ok(ret)
}

fn fill_identity<'a, F>(value: Option<&String>, into: F) -> Result<()>
where
    F: FnOnce(u32) -> capnp::struct_list::Builder<'a, identity::Owned>,
{
    if value.is_none() {
        return Ok(());
    }

    let idents = ident::read(value.unwrap()).chain_err(|| {
        format!("parsing {}", value.unwrap())
    })?;

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

fn fill_priority(mut into: priority::Builder, string: &str) -> Result<()> {
    match string {
        "required" => into.set_required(()),
        "important" => into.set_important(()),
        "standard" => into.set_standard(()),
        "optional" => into.set_optional(()),
        "extra" => into.set_extra(()),
        "source" => into.set_source(()),
        "unknown" => into.set_unknown(()),
        other => bail!("unsupported priority: '{}'", other),
    }

    Ok(())
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
