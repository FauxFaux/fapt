use std::collections::HashMap;

use apt_capnp::item;
use apt_capnp::raw_binary;
use apt_capnp::binary;
use errors::*;
use fields;

use as_u32;
use blank_to_null;
use get_handled_entries;
use fill_identity;

pub fn populate(input: raw_binary::Reader, root: &mut item::Builder) -> Result<()> {
    let mut output = root.borrow().init_binary();

    let handled_entries = get_handled_entries(input.get_entries()?, &fields::HANDLED_FIELDS_BINARY)
        .chain_err(
            || "early parse error finding handled fields (including name)",
        )?;

    let package = if let Some(package) = handled_entries.get("Package") {
        output.set_package(package);
        package.clone()
    } else {
        String::new()
    };

    populate_message(input, output, handled_entries).chain_err(
        || {
            format!("populating binary package '{}'", package)
        },
    )?;

    Ok(())
}

fn populate_message(
    input: raw_binary::Reader,
    mut output: binary::Builder,
    handled_entries: HashMap<String, String>,
) -> Result<()> {

    if let Some(version) = handled_entries.get("Version") {
        output.set_version(version);
    }

    fill_identity(handled_entries.get("Maintainer"), |len| {
        output.borrow().init_maintainer(len)
    }).chain_err(|| "parsing Maintainer")?;

    fill_identity(handled_entries.get("Original-Maintainer"), |len| {
        output.borrow().init_original_maintainer(len)
    }).chain_err(|| "parsing Original-Maintainer")?;

    let mut unparsed = output.init_unparsed();

    let reader = input.get_entries()?;
    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;

        if fields::HANDLED_FIELDS_BINARY.contains(&key) {
            continue;
        }

        let val = reader.get_value()?;

        fields::set_field_binary(key, val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?;
    }

    Ok(())
}
