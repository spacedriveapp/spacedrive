import AsyncStorage from '@react-native-async-storage/async-storage';

export async function getAccessToken() {
	const fetched = await AsyncStorage.getItem('access_token');
	return fetched;
}

export const AUTH_SERVER_URL = __DEV__ ? 'http://localhost:9420' : 'https://auth.spacedrive.com';
