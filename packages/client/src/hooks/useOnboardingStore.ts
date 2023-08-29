import { useSnapshot } from 'valtio';
import { valtioPersist } from '../lib';

export enum UseCase {
	CameraRoll = 'cameraRoll',
	MediaConsumption = 'mediaConsumption',
	MediaCreation = 'mediaCreation',
	CloudBackup = 'cloudBackup',
	Other = 'other'
}

const onboardingStoreDefaults = () => ({
	unlockedScreens: ['alpha'],
	lastActiveScreen: null as string | null,
	useCases: [] as UseCase[],
	grantedFullDiskAccess: false,
	data: {} as Record<string, any> | undefined
});

const appOnboardingStore = valtioPersist('onboarding', onboardingStoreDefaults());

export function useOnboardingStore() {
	return useSnapshot(appOnboardingStore);
}

export function getOnboardingStore() {
	return appOnboardingStore;
}

export function resetOnboardingStore() {
	Object.assign(appOnboardingStore, onboardingStoreDefaults());
}

export function unlockOnboardingScreen(key: string, unlockedScreens: string[] = []) {
	appOnboardingStore.lastActiveScreen = key;
	if (unlockedScreens.includes(key)) {
		appOnboardingStore.unlockedScreens = unlockedScreens;
	} else {
		appOnboardingStore.unlockedScreens = [...unlockedScreens, key];
	}
}
