import cryptoRandomString from 'crypto-random-string';

// NOTE: `crypto` module is not available in RN so this can't be in client
export const generatePassword = (length: number) =>
	cryptoRandomString({ length, type: 'ascii-printable' });
