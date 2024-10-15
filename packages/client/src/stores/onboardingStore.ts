import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solid';

export enum UseCase {
	CameraRoll = 'cameraRoll',
	MediaConsumption = 'mediaConsumption',
	MediaCreation = 'mediaCreation',
	CloudBackup = 'cloudBackup',
	Other = 'other'
}

const onboardingStoreDefaults = () => ({
	unlockedScreens: ['prerelease'],
	lastActiveScreen: null as string | null,
	useCases: [] as UseCase[],
	grantedFullDiskAccess: false,
	data: {} as Record<string, any> | undefined,
	showIntro: true
});

export const onboardingStore = createPersistedMutable(
	'onboarding',
	createMutable(onboardingStoreDefaults())
);

export function useOnboardingStore() {
	return useSolidStore(onboardingStore);
}

export function resetOnboardingStore() {
	Object.assign(onboardingStore, onboardingStoreDefaults());
}

export function unlockOnboardingScreen(key: string, unlockedScreens: string[] = []) {
	onboardingStore.lastActiveScreen = key;
	if (unlockedScreens.includes(key)) {
		onboardingStore.unlockedScreens = unlockedScreens;
	} else {
		onboardingStore.unlockedScreens = [...unlockedScreens, key];
	}
}
