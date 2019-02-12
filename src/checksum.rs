use std::fmt;
use std::io;
use std::io::Read;

use failure::ensure;
use failure::Error;
use hex;
use hex::FromHex;
use sha2::Digest;
use sha2::Sha256;

pub type MD5 = [u8; 16];
pub type SHA256 = [u8; 32];

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct Hashes {
    pub md5: MD5,
    pub sha256: SHA256,
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

pub fn parse_md5(hash: &str) -> Result<MD5, Error> {
    let mut arr = [0u8; 16];
    let v = Vec::from_hex(hash)?;
    ensure!(
        arr.len() == v.len(),
        "a md5 checksum isn't the right length? {}",
        hash
    );
    arr.copy_from_slice(&v);

    Ok(arr)
}

pub fn parse_sha256(hash: &str) -> Result<SHA256, Error> {
    let mut arr = [0u8; 32];

    let v = Vec::from_hex(hash)?;
    ensure!(
        arr.len() == v.len(),
        "a sha256 checksum isn't the right length? {}",
        hash
    );

    arr.copy_from_slice(&v);

    Ok(arr)
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
