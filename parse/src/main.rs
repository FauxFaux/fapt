extern crate capnp;
#[macro_use]
extern crate error_chain;

use capnp::serialize;

mod apt_capnp;
mod errors;
mod fields;

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

    set_priority(
        output.borrow().init_priority(),
        &get_entry(input, "Priority")?,
    );



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

    {
        let reader = input.get_entries()?;
        for i in 0..reader.len() {
            let reader = reader.borrow().get(i);
            let key = reader.get_key()?;

            if fields::HANDLED_FIELDS.contains(&key) {
                continue;
            }

            let val = reader.get_value()?;

            fields::set_field(key, val, &mut output.borrow())?;
        }
    }

    Ok(())
}

fn get_entry(input: apt_capnp::raw_source::Reader, name: &str) -> Result<String> {
    let reader = input.get_entries()?;
    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;
        if name == key {
            return Ok(reader.get_value()?.to_string());
        }
    }

    Ok(String::new())
}

fn set_priority(mut into: apt_capnp::priority::Builder, string: &str) {
    match string {
        "required" => into.set_required(()),
        "important" => into.set_important(()),
        "standard" => into.set_standard(()),
        "optional" => into.set_optional(()),
        "extra" => into.set_extra(()),
        "source" => into.set_source(()),
        _ => unimplemented!(),
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
