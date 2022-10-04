use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not enough bytes were written to the output file")]
    WriteMismatch,
    #[error("tried to run an incorrect step operation")]
    IncorrectStep,
    #[error("there was an error hashing the password")]
    PasswordHash,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("error while encrypting")]
    Encrypt,
    #[error("error while decrypting")]
    Decrypt,
    #[error("nonce length mismatch")]
    NonceLengthMismatch,
}