use std::io;
use std::io::Write;
use std::io::Read;

use failure::Error;
use hex;
use sha2::Digest;
use sha2::Sha256;

use Hashes;

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
