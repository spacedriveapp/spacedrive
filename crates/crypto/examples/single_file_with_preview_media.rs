use std::fs::File;

use sd_crypto::{Protected, primitives::{generate_master_key, generate_salt}, keys::hashing::{HashingAlgorithm, Params}, crypto::stream::{Algorithm, StreamEncryption}, header::{keyslot::{Keyslot, KeyslotVersion}, file::{FileHeader, FileHeaderVersion}, preview_media::{PreviewMediaVersion, PreviewMedia}}};

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

fn encrypt() {
    let password = Protected::new(b"password".to_vec());

    // Open both the source and the output file
    let mut reader = File::open("test").unwrap();
    let mut writer = File::create("test.encrypted").unwrap();

    // This needs to be generated here, otherwise we won't have access to it for encryption
    let master_key = generate_master_key();

    // Create a keyslot to be added to the header
    // The password is cloned as we also need to provide this for the preview media
    let mut keyslots: Vec<Keyslot> = Vec::new();
    keyslots.push(
        Keyslot::new(
            KeyslotVersion::V1,
            ALGORITHM,
            HASHING_ALGORITHM,
            password.clone(),
            &master_key,
        )
        .unwrap(),
    );


    // Ideally this will be generated via the key management system
    let pvm_salt = generate_salt();

    let pvm_media = b"a nice mountain".to_vec();

    let pvm = PreviewMedia::new(
        PreviewMediaVersion::V1,
        ALGORITHM,
        HASHING_ALGORITHM,
        password,
        &pvm_salt,
        &pvm_media,
    )
    .unwrap();

    // Create the header for the encrypted file (and include our preview media)
    let header = FileHeader::new(
        FileHeaderVersion::V1,
        ALGORITHM,
        keyslots,
        None,
        Some(pvm),
    );

    // Write the header to the file
    header.write(&mut writer).unwrap();

    // Use the nonce created by the header to initialise a stream encryption object
    let encryptor = StreamEncryption::new(master_key, &header.nonce, header.algorithm).unwrap();

    // Encrypt the data from the reader, and write it to the writer
    // Use AAD so the header can be authenticated against every block of data
    encryptor
        .encrypt_streams(&mut reader, &mut writer, &header.generate_aad())
        .unwrap();
}

pub fn decrypt_preview_media() {
    let password = Protected::new(b"password".to_vec());

    // Open the encrypted file
    let mut reader = File::open("test.encrypted").unwrap();

    // Deserialize the header, keyslots, etc from the encrypted file
    let (header, _) = FileHeader::deserialize(&mut reader).unwrap();

    // Checks should be made to ensure the file actually contains any preview media
    let pvm = header.preview_media.unwrap();

    // Hash the user's password with the preview media salt
    // This should be done by a key management system
    let hashed_key = pvm.hashing_algorithm.hash(password, pvm.salt).unwrap();

    // Decrypt the preview media
    let media = pvm.decrypt_preview_media(hashed_key).unwrap();

    println!("{:?}", media.expose());
}

fn main() {
    encrypt();

    decrypt_preview_media();
}