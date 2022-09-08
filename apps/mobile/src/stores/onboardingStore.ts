import proxyWithPersist, { PersistStrategy } from 'valtio-persist';

import { StorageEngine } from './utils';

// Might wanna make this a `appStore` so we can add other stuff to it
export const onboardingStore = proxyWithPersist({
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
