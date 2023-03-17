#![cfg(feature = "headers")]

use tokio::fs::File;

use sd_crypto::{
	crypto::Encryptor,
	header::{FileHeader, HeaderObjectName},
	primitives::{FILE_KEYSLOT_CONTEXT, LATEST_FILE_HEADER},
	types::{Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Params, Salt},
	Protected,
};

const MAGIC_BYTES: MagicBytes<6> = MagicBytes::new(*b"crypto");
const OBJECT_IDENTIFIER_CONTEXT: DerivationContext =
	DerivationContext::new("spacedrive 2023-03-16 18:10:47 header object examples");

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

async fn encrypt() {
	let password = Protected::new(b"password".to_vec());

	// Open both the source and the output file
	let mut reader = File::open("test").await.unwrap();
	let mut writer = File::create("test.encrypted").await.unwrap();

	// This needs to be generated here, otherwise we won't have access to it for encryption
	let master_key = Key::generate();

	// These should ideally be done by a key management system
	let content_salt = Salt::generate();
	let hashed_password = HASHING_ALGORITHM
		.hash(password, content_salt, None)
		.unwrap();

	let pvm = b"a nice mountain".to_vec();

	// Create the header for the encrypted file
	let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

	// Create a keyslot to be added to the header
	header
		.add_keyslot(
			HASHING_ALGORITHM,
			content_salt,
			hashed_password,
			master_key.clone(),
			FILE_KEYSLOT_CONTEXT,
		)
		.unwrap();

	header
		.add_object(
			HeaderObjectName::new("PreviewMedia"),
			OBJECT_IDENTIFIER_CONTEXT,
			master_key.clone(),
			&pvm,
		)
		.unwrap();

	// Write the header to the file
	header.write_async(&mut writer, MAGIC_BYTES).await.unwrap();

	// Use the nonce created by the header to initialise a stream encryption object
	let encryptor = Encryptor::new(master_key, header.get_nonce(), header.get_algorithm()).unwrap();

	// Encrypt the data from the reader, and write it to the writer
	// Use AAD so the header can be authenticated against every block of data
	encryptor
		.encrypt_streams_async(&mut reader, &mut writer, &header.get_aad())
		.await
		.unwrap();
}

async fn decrypt_preview_media() {
	let password = Protected::new(b"password".to_vec());

	// Open the encrypted file
	let mut reader = File::open("test.encrypted").await.unwrap();

	// Deserialize the header, keyslots, etc from the encrypted file
	let header = FileHeader::from_reader_async(&mut reader, MAGIC_BYTES)
		.await
		.unwrap();

	let master_key = header
		.decrypt_master_key_with_password(password, FILE_KEYSLOT_CONTEXT)
		.unwrap();

	// Decrypt the preview media
	let media = header
		.decrypt_object(
			HeaderObjectName::new("PreviewMedia"),
			OBJECT_IDENTIFIER_CONTEXT,
			master_key,
		)
		.unwrap();

	println!("{:?}", media.expose());
}

#[tokio::main]
async fn main() {
	encrypt().await;

	decrypt_preview_media().await;
}
