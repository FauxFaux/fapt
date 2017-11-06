use std::fs;
use std::io;
use std::path::Path;

use gpgme::context::Context;
use gpgme::Data;
use gpgme::Protocol;

use tempfile_fast::persistable_tempfile_in;

use errors::*;

pub fn verify_clearsigned<P: AsRef<Path>, Q: AsRef<Path>>(file: P, dest: Q) -> Result<()> {
    let mut ctx = Context::from_protocol(Protocol::OpenPgp)?;
    let from = fs::File::open(file)?;
    let to = persistable_tempfile_in(dest.as_ref().parent().ok_or("full path please")?)?;
    let to_data = Data::from_seekable_stream(to.as_ref()).map_err(
        |e| e.error(),
    )?;
    ctx.verify_opaque(from, to_data)?;
    Ok(())
}
