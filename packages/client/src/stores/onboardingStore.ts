import { useSnapshot } from 'valtio';

import { valtioPersist } from './util';

export enum UseCase {
	CameraRoll = 'cameraRoll',
	MediaConsumption = 'mediaConsumption',
	MediaCreation = 'mediaCreation',
	CloudBackup = 'cloudBackup',
	Other = 'other'
}

const onboardingStoreDefaults = {
	newLibraryName: '',
	unlockedScreens: ['start'],
	lastActiveScreen: null as string | null,
	shouldEncryptLibrary: false,
	algorithm: 'XChaCha20Poly1305',
	hashingAlgorithm: 'Argon2id-s',
	passwordSetToken: null as string | null,
	shareTelemetryDataWithDevelopers: true,
	useCases: [] as UseCase[],
	grantedFullDiskAccess: false
};

const appOnboardingStore = valtioPersist('onboarding', onboardingStoreDefaults);

export function useOnboardingStore() {
	return useSnapshot(appOnboardingStore);
}

export function getOnboardingStore() {
	return appOnboardingStore;
}

export function resetOnboardingStore() {
	for (const key in onboardingStoreDefaults) {
		// @ts-expect-error - TODO: type needs to be fixed
		appOnboardingStore[key] = onboardingStoreDefaults[key];
	}
}

export function unlockOnboardingScreen(key: string, unlockedScreens: string[] = []) {
	appOnboardingStore.lastActiveScreen = key;
	if (unlockedScreens.includes(key)) {
		appOnboardingStore.unlockedScreens = unlockedScreens;
	} else {
		appOnboardingStore.unlockedScreens = [...unlockedScreens, key];
	}
}
