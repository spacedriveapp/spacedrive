use tokio::fs::File;

use sd_crypto::{
	crypto::stream::{Algorithm, StreamEncryption},
	header::{file::FileHeader, keyslot::Keyslot, preview_media::PreviewMediaVersion},
	keys::hashing::{HashingAlgorithm, Params},
	primitives::{generate_master_key, generate_salt, LATEST_FILE_HEADER, LATEST_KEYSLOT},
	Protected,
};

const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

async fn encrypt() {
	let password = Protected::new(b"password".to_vec());

	// Open both the source and the output file
	let mut reader = File::open("test").await.unwrap();
	let mut writer = File::create("test.encrypted").await.unwrap();

	// This needs to be generated here, otherwise we won't have access to it for encryption
	let master_key = generate_master_key();

	// These should ideally be done by a key management system
	let content_salt = generate_salt();
	let hashed_password = HASHING_ALGORITHM
		.hash(password, content_salt, None)
		.unwrap();

	// Create a keyslot to be added to the header
	let keyslots = vec![Keyslot::new(
		LATEST_KEYSLOT,
		ALGORITHM,
		HASHING_ALGORITHM,
		content_salt,
		hashed_password,
		master_key.clone(),
	)
	.await
	.unwrap()];

	let pvm_media = b"a nice mountain".to_vec();

	// Create the header for the encrypted file (and include our preview media)
	let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM, keyslots);

	header
		.add_preview_media(
			PreviewMediaVersion::V1,
			ALGORITHM,
			master_key.clone(),
			&pvm_media,
		)
		.await
		.unwrap();

	// Write the header to the file
	header.write(&mut writer).await.unwrap();

	// Use the nonce created by the header to initialise a stream encryption object
	let encryptor = StreamEncryption::new(master_key, &header.nonce, header.algorithm).unwrap();

	// Encrypt the data from the reader, and write it to the writer
	// Use AAD so the header can be authenticated against every block of data
	encryptor
		.encrypt_streams(&mut reader, &mut writer, &header.generate_aad())
		.await
		.unwrap();
}

async fn decrypt_preview_media() {
	let password = Protected::new(b"password".to_vec());

	// Open the encrypted file
	let mut reader = File::open("test.encrypted").await.unwrap();

	// Deserialize the header, keyslots, etc from the encrypted file
	let (header, _) = FileHeader::from_reader(&mut reader).await.unwrap();

	// Decrypt the preview media
	let media = header.decrypt_preview_media(password).await.unwrap();

	println!("{:?}", media.expose());
}

#[tokio::main]
async fn main() {
	encrypt().await;

	decrypt_preview_media().await;
}
