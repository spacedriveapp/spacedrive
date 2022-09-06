import AsyncStorage from '@react-native-async-storage/async-storage';
import type { ProxyPersistStorageEngine } from 'valtio-persist';

export const StorageEngine: ProxyPersistStorageEngine = {
	getItem: (name) => AsyncStorage.getItem(name),
	setItem: (name, value) => AsyncStorage.setItem(name, value),
	removeItem: (name) => AsyncStorage.removeItem(name),
	getAllKeys: () => AsyncStorage.getAllKeys() as Promise<string[]>
};
