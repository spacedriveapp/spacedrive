use tokio::fs::File;

use sd_crypto::{
	crypto::{Decryptor, Encryptor},
	header::FileHeader,
	keys::Hasher,
	primitives::LATEST_FILE_HEADER,
	types::{Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Params, Salt},
	Protected,
};

const MAGIC_BYTES: MagicBytes<6> = MagicBytes::new(*b"crypto");

const HEADER_KEY_CONTEXT: DerivationContext =
	DerivationContext::new("crypto 2023-03-21 11:24:53 example header key context");

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

const FILE_NAME: &str = "dfskgjh39u4dgsfjk.test";

async fn encrypt() {
	let password = Protected::new(b"password".to_vec());

	// Open both the source and the output file
	let mut reader = File::open(FILE_NAME).await.unwrap();
	let mut writer = File::create(format!("{FILE_NAME}.encrypted"))
		.await
		.unwrap();

	// This needs to be generated here, otherwise we won't have access to it for encryption
	let master_key = Key::generate();

	// These should ideally be done by a key management system
	let content_salt = Salt::generate();
	let hashed_password = Hasher::hash(HASHING_ALGORITHM, password, content_salt, None).unwrap();

	// Create the header for the encrypted file
	let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

	// Create a keyslot to be added to the header
	header
		.add_keyslot(
			HASHING_ALGORITHM,
			content_salt,
			hashed_password,
			master_key.clone(),
			HEADER_KEY_CONTEXT,
		)
		.unwrap();

	// Write the header to the file
	header.write_async(&mut writer, MAGIC_BYTES).await.unwrap();

	// Use the nonce created by the header to initialize a stream encryption object
	let encryptor = Encryptor::new(master_key, header.get_nonce(), header.get_algorithm()).unwrap();

	// Encrypt the data from the reader, and write it to the writer
	// Use AAD so the header can be authenticated against every block of data
	encryptor
		.encrypt_streams_async(&mut reader, &mut writer, header.get_aad().inner())
		.await
		.unwrap();
}

async fn decrypt() {
	let password = Protected::new(b"password".to_vec());

	// Open both the encrypted file and the output file
	let mut reader = File::open(format!("{FILE_NAME}.encrypted")).await.unwrap();
	let mut writer = File::create(format!("{FILE_NAME}.original")).await.unwrap();

	// Deserialize the header, keyslots, etc from the encrypted file
	let header = FileHeader::from_reader_async(&mut reader, MAGIC_BYTES)
		.await
		.unwrap();

	// Decrypt the master key with the user's password
	let master_key = header
		.decrypt_master_key_with_password(password, HEADER_KEY_CONTEXT)
		.unwrap();

	// Initialize a stream decryption object using data provided by the header
	let decryptor = Decryptor::new(master_key, header.get_nonce(), header.get_algorithm()).unwrap();

	// Decrypt data the from the reader, and write it to the writer
	decryptor
		.decrypt_streams_async(&mut reader, &mut writer, header.get_aad().inner())
		.await
		.unwrap();
}

#[tokio::main]
async fn main() {
	File::create(FILE_NAME).await.unwrap();

	encrypt().await;

	decrypt().await;

	tokio::fs::remove_file(FILE_NAME).await.unwrap();
	tokio::fs::remove_file(format!("{FILE_NAME}.encrypted"))
		.await
		.unwrap();
	tokio::fs::remove_file(format!("{FILE_NAME}.original"))
		.await
		.unwrap();
}
