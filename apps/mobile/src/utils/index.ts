import AsyncStorage from '@react-native-async-storage/async-storage';

export async function getTokens() {
	const fetchedToken = await AsyncStorage.getItem('access_token');
	const fetchedRefreshToken = await AsyncStorage.getItem('refresh_token');
	return {
		accessToken: fetchedToken ?? '',
		refreshToken: fetchedRefreshToken ?? ''
	};
}

// export const AUTH_SERVER_URL = __DEV__ ? 'http://localhost:9420' : 'https://auth.spacedrive.com';
export const AUTH_SERVER_URL = 'https://auth.spacedrive.com';
