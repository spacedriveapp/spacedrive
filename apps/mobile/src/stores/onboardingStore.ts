import { useSnapshot } from 'valtio';
import proxyWithPersist, { PersistStrategy } from 'valtio-persist';

import { StorageEngine } from './utils';

// Might wanna rename to `appStore` so we can add other stuff to it
const onboardingStore = proxyWithPersist({
	initialState: {
		showOnboarding: true,
		hideOnboarding: () => {
			onboardingStore.showOnboarding = false;
		}
	},
	persistStrategies: PersistStrategy.SingleFile,
	name: 'sd-onboarding-store',
	version: 0,
	migrations: {},
	getStorage: () => StorageEngine
});

export function useOnboardingStore() {
	const store = useSnapshot(onboardingStore);
	return {
		showOnboarding: store.showOnboarding,
		hideOnboarding: store.hideOnboarding,
		isLoaded: store._persist.loaded
	};
}
