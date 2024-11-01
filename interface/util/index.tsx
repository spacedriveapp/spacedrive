import cryptoRandomString from 'crypto-random-string';

// NOTE: `crypto` module is not available in RN so this can't be in client
export const generatePassword = (length: number) =>
	cryptoRandomString({ length, type: 'ascii-printable' });

export type NonEmptyArray<T> = [T, ...T[]];

export const isNonEmpty = <T,>(input: T[]): input is NonEmptyArray<T> => input.length > 0;
export const isNonEmptyObject = (input: object) => Object.keys(input).length > 0;

export const AUTH_SERVER_URL = 'https://auth.spacedrive.com';
// export const AUTH_SERVER_URL = 'http://localhost:9420';

export function getTokens() {
	if (typeof window === 'undefined') {
		return {
			refreshToken: '',
			accessToken: ''
		};
	}

	const refreshToken: string =
		JSON.parse(window.localStorage.getItem('frontendCookies') ?? '[]')
			.find((cookie: string) => cookie.startsWith('st-refresh-token'))
			?.split('=')[1]
			.split(';')[0] || '';
	const accessToken: string =
		JSON.parse(window.localStorage.getItem('frontendCookies') ?? '[]')
			.find((cookie: string) => cookie.startsWith('st-access-token'))
			?.split('=')[1]
			.split(';')[0] || '';

	return {
		refreshToken,
		accessToken
	};
}
