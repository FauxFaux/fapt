use std::collections::HashMap;

use apt_capnp::binary;
use errors::*;
use fields;

use as_u32;

pub fn populate(output: binary::Builder, map: HashMap<&str, &str>) -> Result<()> {

    let mut unparsed = output.init_unparsed();

    for (key, val) in map.into_iter() {
        if fields::HANDLED_FIELDS_BINARY.contains(&key) {
            continue;
        }

        fields::set_field_binary(key, val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?;
    }

    Ok(())
}
