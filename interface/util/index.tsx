import cryptoRandomString from 'crypto-random-string';
import { nonLibraryClient } from '@sd/client';

// NOTE: `crypto` module is not available in RN so this can't be in client
export const generatePassword = (length: number) =>
	cryptoRandomString({ length, type: 'ascii-printable' });

export type NonEmptyArray<T> = [T, ...T[]];

export const isNonEmpty = <T,>(input: T[]): input is NonEmptyArray<T> => input.length > 0;
export const isNonEmptyObject = (input: object) => Object.keys(input).length > 0;

export const AUTH_SERVER_URL = 'https://auth.spacedrive.com';
// export const AUTH_SERVER_URL = 'http://localhost:9420';

export async function getTokens(): Promise<{ accessToken: string; refreshToken: string }> {
	const tokens = await nonLibraryClient.query(['keys.get']);
	const tokensArray = JSON.parse(tokens);

	const refreshToken: string =
		tokensArray
			.find((cookie: string) => cookie.startsWith('st-refresh-token'))
			?.split('=')[1]
			.split(';')[0] || '';
	const accessToken: string =
		tokensArray
			.find((cookie: string) => cookie.startsWith('st-access-token'))
			?.split('=')[1]
			.split(';')[0] || '';

	return { accessToken, refreshToken };
}
