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
            let mut output = message.init_root::<source::Builder>();
            output.set_package(input.get_package()?);
        }
        serialize::write_message(&mut stdout, &message)?;
    }
}
