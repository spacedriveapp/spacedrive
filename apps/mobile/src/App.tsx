import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { createClient } from '@rspc/client';
import { StatusBar } from 'expo-status-bar';
import React, { useEffect } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';

import { GlobalModals } from './components/modals/GlobalModals';
import {
	ReactNativeTransport,
	queryClient,
	rspc,
	useBridgeQuery,
	useInvalidateQuery
} from './hooks/rspc';
import useCachedResources from './hooks/useCachedResources';
import { getItemFromStorage } from './lib/storage';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { useLibraryStore } from './stores/useLibraryStore';
import { useOnboardingStore } from './stores/useOnboardingStore';
import type { Operations } from './types/bindings';

const client = createClient<Operations>({
	transport: new ReactNativeTransport()
});

const NavigatorTheme: Theme = {
	...DefaultTheme,
	colors: {
		...DefaultTheme.colors,
		background: tw.color('gray-650')
	}
};

function AppContainer() {
	// Enables dark mode, and screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	const isLoadingComplete = useCachedResources();

	const { showOnboarding, hideOnboarding } = useOnboardingStore();

	const { data: libraries } = useBridgeQuery(['library.get']);

	const { switchLibrary, _hasHydrated } = useLibraryStore();

	// Runs when the app is launched
	useEffect(() => {
		async function appLaunch() {
			// Check if the user went through onboarding
			const didOnboarding = await getItemFromStorage('@onboarding');
			// If user did do onboarding, that means they've already have a library

			// Temporarly set the first library to be the current library
			if (libraries && libraries.length > 0) {
				switchLibrary(libraries[0].uuid);
			}

			if (didOnboarding) {
				hideOnboarding();
			}
		}

		appLaunch();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [libraries]);

	// Might need to move _hasHydrated to useCacheResources hook.
	if (!isLoadingComplete && !_hasHydrated) {
		return null;
	} else {
		return (
			<SafeAreaProvider style={tw`flex-1 bg-gray-650`}>
				<GestureHandlerRootView style={tw`flex-1`}>
					<BottomSheetModalProvider>
						<StatusBar style="light" />
						<NavigationContainer theme={NavigatorTheme}>
							{showOnboarding ? <OnboardingNavigator /> : <RootNavigator />}
						</NavigationContainer>
						<GlobalModals />
					</BottomSheetModalProvider>
				</GestureHandlerRootView>
			</SafeAreaProvider>
		);
	}
}

export default function App() {
	return (
		<rspc.Provider client={client} queryClient={queryClient}>
			<>
				<InvalidateQuery />
				<AppContainer />
			</>
		</rspc.Provider>
	);
}

function InvalidateQuery() {
	useInvalidateQuery();
	return null;
}
