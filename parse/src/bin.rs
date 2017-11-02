use apt_capnp::item;
use apt_capnp::raw_binary;
use apt_capnp::binary;
use errors::*;
use fields;

use as_u32;
use blank_to_null;
use get_handled_entries;

pub fn populate(input: raw_binary::Reader, root: &mut item::Builder) -> Result<()> {
    let output = root.borrow().init_binary();

    // TODO: find package name earlier so we can display it

    populate_message(input, output)?;

    Ok(())
}

fn populate_message(input: raw_binary::Reader, mut output: binary::Builder) -> Result<()> {
    let handled_entries =
        get_handled_entries(input.get_entries()?, &fields::HANDLED_FIELDS_BINARY)?;

    if let Some(package) = handled_entries.get("Package") {
        output.set_package(package);
    }

    if let Some(version) = handled_entries.get("Version") {
        output.set_version(version);
    }

    let mut unparsed = output.init_unparsed();

    let reader = input.get_entries()?;
    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;

        if fields::HANDLED_FIELDS_SOURCE.contains(&key) {
            continue;
        }

        let val = reader.get_value()?;

        fields::set_field_binary(key, val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?;
    }

    Ok(())
}
