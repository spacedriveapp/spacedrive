use std::io;

use sd_crypto::{
	crypto::Encryptor,
	header::{FileHeader, HeaderObjectName},
	keys::Hasher,
	primitives::LATEST_FILE_HEADER,
	types::{Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Salt},
	Protected,
};

const MAGIC_BYTES: MagicBytes<6> = MagicBytes::new(*b"crypto");

const HEADER_KEY_CONTEXT: DerivationContext =
	DerivationContext::new("crypto 2023-03-21 11:24:53 example header key context");

const HEADER_OBJECT_CONTEXT: DerivationContext =
	DerivationContext::new("crypto 2023-03-21 11:25:08 example header object context");

const ALGORITHM: Algorithm = Algorithm::default();
const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::default();

const OBJECT_DATA: [u8; 15] = *b"a nice mountain";

fn encrypt<R, W>(reader: &mut R, writer: &mut W)
where
	R: io::Read,
	W: io::Write,
{
	let password = Protected::new(b"password".to_vec());

	// This needs to be generated here, otherwise we won't have access to it for encryption
	let master_key = Key::generate();

	// These should ideally be done by a key management system
	let content_salt = Salt::generate();
	let hashed_password =
		Hasher::hash_password(HASHING_ALGORITHM, password, content_salt, None).unwrap();

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

	header
		.add_object(
			HeaderObjectName::new("FileMetadata"),
			HEADER_OBJECT_CONTEXT,
			master_key.clone(),
			&OBJECT_DATA,
		)
		.unwrap();

	// Write the header to the file
	header.write(writer, MAGIC_BYTES).unwrap();

	// Use the nonce created by the header to initialize an encryptor
	let encryptor = Encryptor::new(master_key, header.get_nonce(), header.get_algorithm()).unwrap();

	// Encrypt the data from the reader, and write it to the writer
	// Use AAD so the header can be authenticated against every block of data
	encryptor
		.encrypt_streams(reader, writer, header.get_aad())
		.unwrap();
}

fn decrypt<R>(reader: &mut R) -> Vec<u8>
where
	R: io::Read + io::Seek,
{
	let password = Protected::new(b"password".to_vec());

	// Deserialize the header from the encrypted file
	let header = FileHeader::from_reader(reader, MAGIC_BYTES).unwrap();

	let master_key = header
		.decrypt_master_key_with_password(password, HEADER_KEY_CONTEXT)
		.unwrap();

	// Decrypt the object
	let object = header
		.decrypt_object(
			HeaderObjectName::new("FileMetadata"),
			HEADER_OBJECT_CONTEXT,
			master_key,
		)
		.unwrap();

	object.into_inner()
}

fn main() {
	// Open both the source and the output file
	let mut source = io::Cursor::new(vec![5u8; 256]);
	let mut dest = io::Cursor::new(vec![]);

	encrypt(&mut source, &mut dest);
	let object_data = decrypt(&mut dest);

	assert_eq!(&object_data, &OBJECT_DATA);
}
