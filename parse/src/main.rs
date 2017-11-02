extern crate capnp;
#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate nom;

use std::collections::HashMap;

use capnp::serialize;

mod apt_capnp;
mod bin;
mod deps;
mod errors;
mod fields;
mod src;
mod vcs;

use apt_capnp::item;
use apt_capnp::entry;
use errors::*;

quick_main!(run);

fn run() -> Result<()> {
    let stdin = ::std::io::stdin();
    let mut stdin = stdin.lock();

    let stdout = ::std::io::stdout();
    let mut stdout = stdout.lock();

    loop {
        let input = serialize::read_message(&mut stdin, capnp::message::ReaderOptions::new())?;

        let input = input.get_root::<item::Reader>()?;

        let mut message = capnp::message::Builder::new_default();

        {
            let mut root = message.init_root::<item::Builder>();

            match input.which()? {
                item::End(()) => return Ok(()),
                item::Source(_) | item::Binary(_) => {
                    bail!("unexpected item type in stream: already processed?")
                }
                item::RawSource(e) => src::populate(e?, &mut root)?,
                item::RawBinary(e) => bin::populate(e?, &mut root)?,
            };
        }

        serialize::write_message(&mut stdout, &message)?;
    }
}

fn get_handled_entries(
    reader: capnp::struct_list::Reader<entry::Owned>,
    handled: &[&str],
) -> Result<HashMap<String, String>> {
    let mut ret = HashMap::with_capacity(handled.len());

    for i in 0..reader.len() {
        let reader = reader.borrow().get(i);
        let key = reader.get_key()?;
        if !handled.contains(&key) {
            continue;
        }

        ret.insert(key.to_string(), reader.get_value()?.to_string());
    }

    Ok(ret)
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
