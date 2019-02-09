use std::fs;
use std::io;
use std::path::Path;

use failure::format_err;
use failure::Error;
use failure::ResultExt;
use gpgrv::Keyring;
use tempfile_fast::PersistableTempFile;

pub struct GpgClient<'k> {
    keyring: &'k Keyring,
}

impl<'k> GpgClient<'k> {
    pub fn new(keyring: &Keyring) -> GpgClient {
        GpgClient { keyring }
    }

    pub fn verify_clearsigned<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        file: P,
        dest: Q,
    ) -> Result<(), Error> {
        let from = fs::File::open(file).with_context(|_| format_err!("opening input file"))?;
        let to = PersistableTempFile::new_in(
            dest.as_ref()
                .parent()
                .ok_or_else(|| format_err!("full path please"))?,
        )
        .with_context(|_| format_err!("creating temporary file"))?;

        gpgrv::verify_message(io::BufReader::new(from), &to, &self.keyring)?;

        to.persist_by_rename(dest)
            .map_err(|e| e.error)
            .with_context(|_| format_err!("persisting output file"))?;

        Ok(())
    }

    pub fn verify_detached<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(
        &mut self,
        file: P,
        signature: Q,
        dest: R,
    ) -> Result<(), Error> {
        gpgrv::verify_detached(
            io::BufReader::new(
                fs::File::open(signature)
                    .with_context(|_| format_err!("opening signature file"))?,
            ),
            fs::File::open(file.as_ref()).with_context(|_| format_err!("opening input file"))?,
            &self.keyring,
        )?;
        fs::copy(file, dest)?;
        Ok(())
    }
}
