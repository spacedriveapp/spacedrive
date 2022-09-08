import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { createClient } from '@rspc/client';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';

import { GlobalModals } from './components/modals/GlobalModals';
import { ReactNativeTransport, queryClient, rspc, useInvalidateQuery } from './hooks/rspc';
import useCachedResources from './hooks/useCachedResources';
import { getItemFromStorage } from './lib/storage';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { useOnboardingStore } from './stores/useOnboardingStore';
import type { Operations } from './types/bindings';

const client = createClient<Operations>({
	transport: new ReactNativeTransport()
});

const NavigatorTheme: Theme = {
	...DefaultTheme,
	colors: {
		...DefaultTheme.colors,
		background: '#08090D'
	}
};

export default function App() {
	// Enables dark mode, and screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	const isLoadingComplete = useCachedResources();

	const { showOnboarding, hideOnboarding } = useOnboardingStore();

	// Runs when the app is launched
	useEffect(() => {
		getItemFromStorage('@onboarding').then((value) => {
			value && hideOnboarding();
		});
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	if (!isLoadingComplete) {
		return null;
	} else {
		return (
			<rspc.Provider client={client} queryClient={queryClient}>
				<>
					<InvalidateQuery />
					<SafeAreaProvider style={tw`flex-1 bg-black`}>
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
				</>
			</rspc.Provider>
		);
	}
}

function InvalidateQuery() {
	useInvalidateQuery();
	return null;
}
