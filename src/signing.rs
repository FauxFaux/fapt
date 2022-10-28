use std::fs;
use std::io;
use std::path::Path;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use gpgrv::Keyring;
use tempfile_fast::PersistableTempFile;

pub struct GpgClient<'k> {
    keyring: &'k Keyring,
}

impl<'k> GpgClient<'k> {
    pub fn new(keyring: &Keyring) -> GpgClient {
        GpgClient { keyring }
    }

    pub fn read_clearsigned<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        file: P,
        dest: Q,
        verify: bool,
    ) -> Result<(), Error> {
        let from = fs::File::open(file).with_context(|| anyhow!("opening input file"))?;
        let to = PersistableTempFile::new_in(
            dest.as_ref()
                .parent()
                .ok_or_else(|| anyhow!("full path please"))?,
        )
        .with_context(|| anyhow!("creating temporary file"))?;

        let reader = io::BufReader::new(from);
        if verify {
            gpgrv::verify_message(reader, &to, &self.keyring)?;
        } else {
            gpgrv::read_doc(reader, &to)?;
        }

        to.persist_by_rename(dest)
            .map_err(|e| e.error)
            .with_context(|| anyhow!("persisting output file"))?;

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
                fs::File::open(signature).with_context(|| anyhow!("opening signature file"))?,
            ),
            fs::File::open(file.as_ref()).with_context(|| anyhow!("opening input file"))?,
            &self.keyring,
        )?;
        fs::copy(file, dest)?;
        Ok(())
    }
}
