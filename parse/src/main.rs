extern crate capnp;
#[macro_use]
extern crate error_chain;

use capnp::serialize;

mod apt_capnp;
mod errors;

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
    {
        let reader = input.get_files()?;
        let mut builder = output.init_files(reader.len());
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
