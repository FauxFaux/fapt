use std::fmt;
use std::io;
use std::io::Read;

use failure::ensure;
use failure::Error;
use hex;
use sha2::Digest;
use sha2::Sha256;

#[derive(Copy, Clone)]
pub struct Hashes {
    pub md5: [u8; 16],
    pub sha256: [u8; 32],
}

impl fmt::Debug for Hashes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "md5:{} sha256:{}",
            hex::encode(self.md5),
            hex::encode(self.sha256)
        )
    }
}

// TODO: also check the md5?
pub fn validate<R: Read>(mut file: R, checksum: Hashes) -> Result<(), Error> {
    let mut func = Sha256::default();
    io::copy(&mut file, &mut func)?;
    let result = func.result();
    ensure!(
        checksum.sha256 == result.as_slice(),
        "checksum mismatch: expected: {}, actual: {}",
        hex::encode(checksum.sha256),
        hex::encode(result.as_slice())
    );
    Ok(())
}
