import { useSnapshot } from 'valtio';

import { valtioPersist } from './util';

export enum UseCase {
	CameraRoll = 'cameraRoll',
	MediaConsumption = 'mediaConsumption',
	MediaCreation = 'mediaCreation',
	CloudBackup = 'cloudBackup',
	Other = 'other'
}

const appOnboardingStore = valtioPersist('onboarding', {
	newLibraryName: '',
	unlockedScreens: ['start'],
	lastActiveScreen: null as string | null,
	shouldEncryptLibrary: false,
	hasSetPassword: false,
	shareTelemetryDataWithDevelopers: true,
	useCases: [] as UseCase[],
	grantedFullDiskAccess: false
});

export function useOnboardingStore() {
	return useSnapshot(appOnboardingStore);
}

export function getOnboardingStore() {
	return appOnboardingStore;
}

export function unlockOnboardingScreen(key: string, unlockedScreens: string[] = []) {
	appOnboardingStore.lastActiveScreen = key;
	if (unlockedScreens.includes(key)) {
		appOnboardingStore.unlockedScreens = unlockedScreens;
	} else {
		appOnboardingStore.unlockedScreens = [...unlockedScreens, key];
	}
}
