use std::collections::HashMap;

use apt_capnp::raw_binary;
use apt_capnp::binary;
use errors::*;
use fields;

use as_u32;
use blank_to_null;
use get_handled_entries;
use fill_identity;

pub fn populate(input: raw_binary::Reader, output: binary::Builder, handled_entries: HashMap<String, String>) -> Result<()> {
    populate_message(input, output, handled_entries)?;

    Ok(())
}

fn populate_message(
    input: raw_binary::Reader,
    output: binary::Builder,
    handled_entries: HashMap<String, String>,
) -> Result<()> {

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
