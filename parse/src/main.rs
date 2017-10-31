extern crate capnp;
#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate nom;

use std::collections::HashMap;

use capnp::serialize;

mod apt_capnp;
mod deps;
mod errors;
mod fields;
mod vcs;

use apt_capnp::raw_source;
use apt_capnp::source;
use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let stdin = ::std::io::stdin();
    let mut stdin = stdin.lock();

    let stdout = ::std::io::stdout();
    let mut stdout = stdout.lock();

    loop {
        let input = serialize::read_message(&mut stdin, capnp::message::ReaderOptions::new())?;
        let input = input.get_root::<raw_source::Reader>()?;
        let mut message = capnp::message::Builder::new_default();
        {
            let output = message.init_root::<source::Builder>();
            populate_message(input, output)?;
        }

        serialize::write_message(&mut stdout, &message)?;
    }
}

fn populate_message(input: raw_source::Reader, mut output: source::Builder) -> Result<()> {
    output.set_package(input.get_package()?);
    output.set_version(input.get_version()?);
    output.set_index(input.get_index()?);

    let handled_entries = get_handled_entries(input)?;

    if let Some(priority) = handled_entries.get("Priority") {
        set_priority(output.borrow().init_priority(), priority);
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

    set_format(output.borrow().init_format(), &handled_entries["Format"]);

    if let Some(list) = handled_entries.get("Package-List") {
        let lines: Vec<&str> = list.split('\n').map(|x| x.trim()).collect();
        let mut builder = output.borrow().init_binaries(as_u32(lines.len()));
        for (i, line) in lines.into_iter().enumerate() {
            let mut builder = builder.borrow().get(as_u32(i));
            let parts: Vec<&str> = line.split(' ').collect();
            builder.set_name(parts[0]);
            builder.set_style(parts[1]);
            builder.set_section(parts[2]);
            set_priority(builder.borrow().init_priority(), parts[3]);

            if parts.len() > 4 {
                let mut builder = builder.init_extras(as_u32(parts.len() - 4));
                for (i, part) in parts[4..].iter().enumerate() {
                    builder.set(as_u32(i), part);
                }
            }
        }
    }

    {
        let reader = input.get_files()?;
        let mut builder = output.borrow().init_files(reader.len());
        for i in 0..reader.len() {
            let reader = reader.borrow().get(i);
            let mut builder = builder.borrow().get(i);
            blank_to_null(reader.get_name()?, |x| builder.set_name(x));
            builder.set_size(reader.get_size());
            blank_to_null(reader.get_md5()?, |x| builder.set_md5(x));
            blank_to_null(reader.get_sha1()?, |x| builder.set_sha1(x));
            blank_to_null(reader.get_sha256()?, |x| builder.set_sha256(x));
            blank_to_null(reader.get_sha512()?, |x| builder.set_sha512(x));
        }
    }

    vcs::extract(&handled_entries, &mut output.borrow())?;

    {
        let reader = input.get_entries()?;
        for i in 0..reader.len() {
            let reader = reader.borrow().get(i);
            let key = reader.get_key()?;

            if fields::HANDLED_FIELDS.contains(&key) {
                continue;
            }

            let val = reader.get_value()?;

            if let Err(e) = fields::set_field(key, val, &mut output.borrow()) {
                use std::io::Write;
                write!(std::io::stderr(), "field: {:?}\n", e)?;
            }
        }
    }

    Ok(())
}

fn get_handled_entries(input: apt_capnp::raw_source::Reader) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(fields::HANDLED_FIELDS.len());

    let reader = input.get_entries()?;
    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;
        if !fields::HANDLED_FIELDS.contains(&key) {
            continue;
        }

        ret.insert(key.to_string(), reader.get_value()?.to_string());
    }

    Ok(ret)
}

fn set_priority(mut into: apt_capnp::priority::Builder, string: &str) {
    match string {
        "required" => into.set_required(()),
        "important" => into.set_important(()),
        "standard" => into.set_standard(()),
        "optional" => into.set_optional(()),
        "extra" => into.set_extra(()),
        "source" => into.set_source(()),
        other => panic!("unsupported priority: '{}'", other),
    }
}

fn set_format(mut into: apt_capnp::source::format::Builder, string: &str) {
    match string {
        "3.0 (quilt)" => into.set_quilt3dot0(()),
        "1.0" => into.set_original(()),
        "3.0 (git)" => into.set_git3dot0(()),
        "3.0 (native)" => into.set_native3dot0(()),
        other => panic!("unsupported source format: '{}'", other),
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
