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

use apt_capnp::index_file;
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
                item::Package(_) => {
                    bail!("unexpected item type in stream: already processed?")
                }
                item::RawSource(input) => {
                    let input = input?;
                    let handled = get_handled_entries(input.get_entries()?, &fields::HANDLED_FIELDS_SOURCE)?;
                    fill_package(&mut package, parse_index(input.get_index()?)?, &handled)?;
                    src::populate(input, package.init_style().init_source(), handled)?;
                },
                item::RawBinary(input) => {
                    let input = input?;
                    let handled = get_handled_entries(input.get_entries()?, &fields::HANDLED_FIELDS_BINARY)?;
                    fill_package(&mut package, input.get_index()?, &handled)?;
                    bin::populate(input, package.init_style().init_binary(), handled)?;
                }
            };
        }

        serialize::write_message(&mut stdout, &message)?;
    }
}

fn fill_package(output: &mut package::Builder, index: index_file::Reader, handled_entries: &HashMap<String, String>) -> Result<()> {

    if let Some(name) = handled_entries.get("Package") {
        output.set_name(name);
    }

    if let Some(version) = handled_entries.get("Version") {
        output.set_version(version);
    }

    output.set_index(index);

    if let Some(priority) = handled_entries.get("Priority") {
        fill_priority(output.borrow().init_priority(), priority)
            .chain_err(|| "top-level priority")?;
    }

    {
        let mut parts: Vec<&str> = handled_entries["Architecture"]
            .split(' ')
            .map(|x| x.trim())
            .collect();
        parts.sort();

        let mut builder = output.borrow().init_arch(as_u32(parts.len()));
        for (i, part) in parts.into_iter().enumerate() {
            builder.set(as_u32(i), part);
        }
    }

    fill_identity(handled_entries.get("Maintainer"), |len| {
        output.borrow().init_maintainer(len)
    }).chain_err(|| "parsing Maintainer")?;

    fill_identity(handled_entries.get("Original-Maintainer"), |len| {
        output.borrow().init_original_maintainer(len)
    }).chain_err(|| "parsing Original-Maintainer")?;

    Ok(())
}

fn parse_index(index: &str) -> Result<index_file::Reader> {
    unimplemented!()
}

fn get_handled_entries(
    reader: capnp::struct_list::Reader<entry::Owned>,
    handled: &[&str],
) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(handled.len());

    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;
        if !handled.contains(&key) {
            continue;
        }

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

    let idents = ident::read(value.unwrap())
        .chain_err(|| format!("parsing {}", value.unwrap()))?;

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
