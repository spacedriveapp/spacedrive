use std::fs::File;

use sd_crypto::{
	crypto::stream::{Algorithm, StreamEncryption},
	header::{
		file::{FileHeader, FileHeaderVersion},
		keyslot::{Keyslot, KeyslotVersion},
		metadata::{Metadata, MetadataVersion},
	},
	keys::hashing::{HashingAlgorithm, Params},
	primitives::{generate_master_key, generate_salt},
	Protected,
};
use serde::{Deserialize, Serialize};

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

#[derive(Serialize, Deserialize)]
pub struct FileInformation {
	pub file_name: String,
}

fn encrypt() {
	let embedded_metadata = FileInformation {
		file_name: "filename.txt".to_string(),
	};

	let password = Protected::new(b"password".to_vec());

	// Open both the source and the output file
	let mut reader = File::open("test").unwrap();
	let mut writer = File::create("test.encrypted").unwrap();

	// This needs to be generated here, otherwise we won't have access to it for encryption
	let master_key = generate_master_key();

	// Create a keyslot to be added to the header
	// The password is cloned as we also need to provide this for the metadata
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
	let md_salt = generate_salt();

	let md = Metadata::new(
		MetadataVersion::V1,
		ALGORITHM,
		HASHING_ALGORITHM,
		password,
		&md_salt,
		&embedded_metadata,
	)
	.unwrap();

	// Create the header for the encrypted file (and include our metadata)
	let header = FileHeader::new(FileHeaderVersion::V1, ALGORITHM, keyslots, Some(md), None);

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

pub fn decrypt_metadata() {
	let password = Protected::new(b"password".to_vec());

	// Open the encrypted file
	let mut reader = File::open("test.encrypted").unwrap();

	// Deserialize the header, keyslots, etc from the encrypted file
	let (header, _) = FileHeader::deserialize(&mut reader).unwrap();

	// Checks should be made to ensure the file actually contains any metadata
	let metadata = header.metadata.unwrap();

	// Hash the user's password with the metadata salt
	// This should be done by a key management system
	let hashed_key = metadata
		.hashing_algorithm
		.hash(password, metadata.salt)
		.unwrap();

	// Decrypt the metadata
	let file_info: FileInformation = metadata.decrypt_metadata(hashed_key).unwrap();

	println!("file name: {}", file_info.file_name);
}

fn main() {
	encrypt();

	decrypt_metadata();
}
