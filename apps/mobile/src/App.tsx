import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { createClient } from '@rspc/client';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';
import { useSnapshot } from 'valtio';

import { GlobalModals } from './containers/modals/GlobalModals';
import {
	ReactNativeTransport,
	queryClient,
	rspc,
	useBridgeQuery,
	useInvalidateQuery
} from './hooks/rspc';
import useCachedResources from './hooks/useCachedResources';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { libraryStore } from './stores/libraryStore';
import { onboardingStore } from './stores/onboardingStore';
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

	const { showOnboarding } = useSnapshot(onboardingStore);

	const { data: libraries } = useBridgeQuery(['library.list'], {
		onError(err) {
			console.error(err);
		}
	});

	const { _persist, switchLibrary } = useSnapshot(libraryStore);

	console.log('persisted?', _persist.loaded);

	// Runs when the app is launched
	useEffect(() => {
		// Temporarly set the first library to be the current library
		if (libraries && libraries.length > 0) {
			switchLibrary(libraries[0].uuid);
		}
	}, [libraries, showOnboarding, switchLibrary]);

	// Might need to move _persist.loaded to useCacheResources hook.
	if (!isLoadingComplete || !_persist.loaded) {
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
