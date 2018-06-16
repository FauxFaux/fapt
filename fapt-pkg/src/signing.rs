use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;

use failure::Error;
use failure::ResultExt;
use gpgme::context::Context;
use gpgme::results::VerificationResult;
use gpgme::Data;
use gpgme::Protocol;
use tempdir::TempDir;
use tempfile_fast::PersistableTempFile;

pub struct GpgClient {
    ctx: Context,
    _root: TempDir,
}

impl GpgClient {
    pub fn new<P: AsRef<Path>>(keyring_paths: &[P]) -> Result<Self, Error> {
        let dir = TempDir::new("fapt-gpgme")
            .with_context(|_| format_err!("creating temporary directory"))?;
        let pubring = fs::File::create(dir.as_ref().join("pubring.gpg"))
            .with_context(|_| format_err!("populating temporary directory"))?;
        concatenate_keyrings_into(keyring_paths, pubring)
            .with_context(|_| format_err!("generating temporary keyring"))?;

        let mut ctx = Context::from_protocol(Protocol::OpenPgp)
            .with_context(|_| format_err!("starting gpg"))?;

        ctx.set_engine_home_dir(
            dir.as_ref()
                .to_str()
                .ok_or_else(|| format_err!("tmpdir must be valid utf-8 for no real reason"))?,
        ).with_context(|_| format_err!("informing gpg about our temporary directory"))?;

        Ok(GpgClient { ctx, _root: dir })
    }

    pub fn verify_clearsigned<P: AsRef<Path>, Q: AsRef<Path>>(
        &mut self,
        file: P,
        dest: Q,
    ) -> Result<(), Error> {
        let from = fs::File::open(file).with_context(|_| format_err!("opening input file"))?;
        let to = PersistableTempFile::new_in(
            dest.as_ref()
                .parent()
                .ok_or_else(|| format_err!("full path please"))?,
        ).with_context(|_| format_err!("creating temporary file"))?;

        let result = self
            .ctx
            .verify_opaque(
                from,
                Data::from_seekable_stream(to.as_ref())
                    .map_err(|e| e.error())
                    .with_context(|_| format_err!("creating output stream"))?,
            )
            .with_context(|_| format_err!("verifying"))?;

        validate_signature(&result)?;

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
        let result = self.ctx.verify_detached(
            fs::File::open(signature).with_context(|_| format_err!("opening signature file"))?,
            fs::File::open(file.as_ref()).with_context(|_| format_err!("opening input file"))?,
        )?;
        validate_signature(&result)?;
        fs::copy(file, dest)?;
        Ok(())
    }
}

/// Oh yes, you read that right. We literally cat the files together and pray.
/// There's no error handling. Not at all. God be with you.
fn concatenate_keyrings_into<P: AsRef<Path>, W: Write>(
    keyring_paths: &[P],
    mut pubring: W,
) -> Result<(), Error> {
    for keyring in keyring_paths {
        io::copy(&mut fs::File::open(keyring)?, &mut pubring)?;
    }
    Ok(())
}

fn validate_signature(result: &VerificationResult) -> Result<(), Error> {
    ensure!(
        result.signatures().next().is_some(),
        "there are no signatures"
    );

    for (i, sig) in result.signatures().enumerate() {
        if !sig.status().is_ok() {
            bail!("signature {} is invalid: {:?}", i, sig.status());
        }
    }

    Ok(())
}
