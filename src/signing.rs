use std::fmt;
use std::fs;
use std::path::Path;

use failure::format_err;
use failure::Error;
use failure::ResultExt;
use gpgrv::Keyring;
use tempfile_fast::PersistableTempFile;

pub struct GpgClient {
    keyring: Keyring,
}

impl GpgClient {
    pub fn new<P: AsRef<Path> + fmt::Debug>(keyring_paths: &[P]) -> Result<Self, Error> {
        let mut keyring = Keyring::new();

        for keyring_path in keyring_paths {
            keyring.append_keys_from(
                fs::File::open(keyring_path)
                    .with_context(|_| format_err!("opening keyring {:?}", keyring_path))?,
            )?;
        }

        Ok(GpgClient { keyring })
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

        gpgrv::verify_message(gpgrv::ManyReader::new(from), &to, &self.keyring)?;

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
            fs::File::open(signature).with_context(|_| format_err!("opening signature file"))?,
            fs::File::open(file.as_ref()).with_context(|_| format_err!("opening input file"))?,
            &self.keyring,
        )?;
        fs::copy(file, dest)?;
        Ok(())
    }
}
