use std::collections::HashMap;

use errors::*;
use fields;

#[cfg(capnp)]
use fill_dep;
use yes_no;

#[cfg(capnp)]
pub fn populate<'a>(
    mut output: binary::Builder,
    map: &mut HashMap<&'a str, &str>,
) -> Result<(Vec<&'a str>, Vec<String>)> {
    let allowed_errors = Vec::new();

    {
        let mut builder = output.borrow().init_file();
        if let Some(s) = map.remove("Filename") {
            builder.set_name(s);
        }

        if let Some(s) = map.remove("Size") {
            builder.set_size(s.parse()?);
        }

        if let Some(s) = map.remove("MD5sum") {
            builder.set_md5(s);
        }

        if let Some(s) = map.remove("SHA1") {
            builder.set_sha1(s);
        }

        if let Some(s) = map.remove("SHA256") {
            builder.set_sha256(s);
        }

        if let Some(s) = map.remove("SHA512") {
            builder.set_sha512(s);
        }
    }

    if let Some(s) = map.remove("Essential") {
        output.set_essential(yes_no(s)?);
    }

    if let Some(s) = map.remove("Build-Essential") {
        output.set_build_essential((yes_no(s)?));
    }

    if let Some(s) = map.remove("Installed-Size") {
        output.set_installed_size(s.parse()?);
    }

    if let Some(text) = map.remove("Description") {
        output.set_description(text);
    } else if let Some(text) = map.remove("Description-en") {
        output.set_description(text);
    } else if let Some(text) = map.remove("Description-en_GB") {
        output.set_description(text);
    }

    // Validating this doesn't make sense: it's the md5 of the translation's description
    map.remove("Description-md5");

    fill_dep(map, "Depends", |len| output.borrow().init_depends(len))?;

    fill_dep(map, "Recommends", |len| {
        output.borrow().init_recommends(len)
    })?;

    fill_dep(map, "Suggests", |len| output.borrow().init_suggests(len))?;

    fill_dep(map, "Enhances", |len| output.borrow().init_enhances(len))?;

    fill_dep(map, "Pre-Depends", |len| {
        output.borrow().init_pre_depends(len)
    })?;

    fill_dep(map, "Breaks", |len| output.borrow().init_breaks(len))?;

    fill_dep(map, "Conflicts", |len| output.borrow().init_conflicts(len))?;

    fill_dep(map, "Replaces", |len| output.borrow().init_replaces(len))?;

    fill_dep(map, "Provides", |len| output.borrow().init_provides(len))?;

    let mut unparsed = output.init_unparsed();

    let mut unrecognised_fields = Vec::new();
    for (key, val) in map {
        if fields::set_field_binary(key, val, &mut unparsed)
            .chain_err(|| format!("setting extra field {}", key))?
        {
            unrecognised_fields.push(*key);
        }
    }

    Ok((unrecognised_fields, allowed_errors))
}
