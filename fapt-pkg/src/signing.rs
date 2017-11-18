use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;

use gpgme::context::Context;
use gpgme::results::VerificationResult;
use gpgme::Data;
use gpgme::Protocol;

use tempdir::TempDir;
use tempfile_fast::persistable_tempfile_in;

use errors::*;

pub struct GpgClient {
    ctx: Context,
    _root: TempDir,
}

impl GpgClient {
    pub fn new<P: AsRef<Path>>(keyring_paths: &[P]) -> Result<Self> {
        let dir = TempDir::new("fapt-gpgme").chain_err(|| "creating temporary directory")?;
        let pubring = fs::File::create(dir.as_ref().join("pubring.gpg"))
            .chain_err(|| "populating temporary directory")?;
        concatenate_keyrings_into(keyring_paths, pubring)
            .chain_err(|| "generating temporary keyring")?;

        let mut ctx = Context::from_protocol(Protocol::OpenPgp).chain_err(|| "starting gpg")?;

        ctx.set_engine_home_dir(dir.as_ref()
            .to_str()
            .ok_or("tmpdir must be valid utf-8 for no real reason")?)
            .chain_err(|| "informing gpg about our temporary directory")?;

        Ok(GpgClient { ctx, _root: dir })
    }


    pub fn verify_clearsigned<P: AsRef<Path>, Q: AsRef<Path>>(
        &mut self,
        file: P,
        dest: Q,
    ) -> Result<()> {
        let from = fs::File::open(file).chain_err(|| "opening input file")?;
        let to = persistable_tempfile_in(dest.as_ref().parent().ok_or("full path please")?)
            .chain_err(|| "creating temporary file")?;

        let result = self.ctx
            .verify_opaque(
                from,
                Data::from_seekable_stream(to.as_ref())
                    .map_err(|e| e.error())
                    .chain_err(|| "creating output stream")?,
            )
            .chain_err(|| "verifying")?;

        validate_signature(&result)?;

        // Slightly racy, but not unsafe.
        if dest.as_ref().exists() {
            fs::remove_file(dest.as_ref()).chain_err(|| "removing output file")?;
        }

        to.persist_noclobber(dest)
            .chain_err(|| "persisting output file")?;

        Ok(())
    }

    pub fn verify_detached<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(
        &mut self,
        file: P,
        signature: Q,
        dest: R,
    ) -> Result<()> {
        let result = self.ctx.verify_detached(
            fs::File::open(signature).chain_err(|| "opening signature file")?,
            fs::File::open(file.as_ref()).chain_err(|| "opening input file")?,
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
) -> Result<()> {
    for keyring in keyring_paths {
        io::copy(&mut fs::File::open(keyring)?, &mut pubring)?;
    }
    Ok(())
}

fn validate_signature(result: &VerificationResult) -> Result<()> {
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
