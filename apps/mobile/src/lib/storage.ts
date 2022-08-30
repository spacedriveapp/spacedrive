import AsyncStorage from '@react-native-async-storage/async-storage';

export const setItemToStorage = async (key: string, value: string | null) => {
	if (value === null) return false;
	try {
		await AsyncStorage.setItem(key, value);
		return true;
	} catch (e: any) {
		// saving error
		console.log('Error', e);
		return false;
	}
};

export const getItemFromStorage = async (key: string) => {
	try {
		const value = await AsyncStorage.getItem(key);
		if (value !== null) {
			return value;
		}
		return undefined;
	} catch (e: any) {
		console.log('Error', e);

		return undefined;
	}
};

// For Objects

export const setObjStorage = async (key: string, value: any) => {
	try {
		await AsyncStorage.setItem(key, JSON.stringify(value));
		return true;
	} catch (e: any) {
		// saving error
		console.log('Error', e);

		return false;
	}
};

export const getObjFromStorage = async (key: string) => {
	try {
		const value = await AsyncStorage.getItem(key);
		if (value !== null) {
			return JSON.parse(value);
		}
		return null;
	} catch (e: any) {
		// error reading value
		console.log('Error', e);

		return null;
	}
};

export async function removeFromStorage(key: string) {
	try {
		await AsyncStorage.removeItem(key);
	} catch (e: any) {
		// remove error
		console.log('Error', e);
	}
}

// We also have Merge, getAllKeys, Multimerge / Multiget etc..
// https://react-native-async-storage.github.io/async-storage/docs/api

// Safe Storage
// ???
