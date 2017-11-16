use std::io::Read;

use hex;
use sha2::Digest;
use sha2::Sha256;

use errors::*;
use Hashes;

// TODO: also check the md5?
pub fn validate<R: Read>(mut file: R, checksum: Hashes) -> Result<()> {
    let result = Sha256::digest_reader(&mut file)?;
    ensure!(
        checksum.sha256 == result.as_slice(),
        "checksum mismatch: expected: {}, actual: {}",
        hex::encode(checksum.sha256),
        hex::encode(result.as_slice())
    );
    Ok(())
}
