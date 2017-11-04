use std::collections::HashMap;

use apt_capnp::raw_binary;
use apt_capnp::binary;
use errors::*;
use fields;

use as_u32;
use blank_to_null;

pub fn populate(
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
