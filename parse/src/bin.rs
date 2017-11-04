use std::collections::HashMap;

use apt_capnp::binary;
use errors::*;
use fields;

use fill_dep;

pub fn populate(mut output: binary::Builder, map: HashMap<&str, &str>) -> Result<()> {
    {
        let mut builder = output.borrow().init_file();
        if let Some(s) = map.get("Filename") {
            builder.set_name(s);
        }

        if let Some(s) = map.get("Size") {
            builder.set_size(s.parse()?);
        }

        if let Some(s) = map.get("MD5sum") {
            builder.set_md5(s);
        }

        if let Some(s) = map.get("SHA1") {
            builder.set_sha1(s);
        }

        if let Some(s) = map.get("SHA256") {
            builder.set_sha256(s);
        }

        if let Some(s) = map.get("SHA512") {
            builder.set_sha512(s);
        }
    }

    if let Some(s) = map.get("Essential") {
        output.set_essential(s.parse()?);
    }

    if let Some(s) = map.get("Build-Essential") {
        output.set_build_essential(s.parse()?);
    }

    if let Some(s) = map.get("Installed-Size") {
        output.set_installed_size(s.parse()?);
    }

    // TODO: description

    fill_dep(&map, "Depends", |len| output.borrow().init_depends(len))?;

    fill_dep(
        &map,
        "Recommends",
        |len| output.borrow().init_recommends(len),
    )?;

    fill_dep(&map, "Suggests", |len| output.borrow().init_suggests(len))?;

    fill_dep(&map, "Enhances", |len| output.borrow().init_enhances(len))?;

    fill_dep(
        &map,
        "Pre-Depends",
        |len| output.borrow().init_pre_depends(len),
    )?;

    fill_dep(&map, "Breaks", |len| output.borrow().init_breaks(len))?;

    fill_dep(&map, "Conflicts", |len| output.borrow().init_conflicts(len))?;

    fill_dep(&map, "Replaces", |len| output.borrow().init_replaces(len))?;

    fill_dep(&map, "Provides", |len| output.borrow().init_provides(len))?;

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
