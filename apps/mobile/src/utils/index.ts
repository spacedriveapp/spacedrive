import AsyncStorage from '@react-native-async-storage/async-storage';

export async function getAccessToken() {
	const fetched = await AsyncStorage.getItem('supertokens-rn-front-token-key');
	return fetched;
}
