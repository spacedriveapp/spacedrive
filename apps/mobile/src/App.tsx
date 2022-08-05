import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import React, { useEffect } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';

import useCachedResources from './hooks/useCachedResources';
import { getItemFromStorage } from './lib/storage';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { useOnboardingStore } from './stores/useOnboardingStore';

//
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
			<SafeAreaProvider style={tw`flex-1 bg-black`}>
				<GestureHandlerRootView style={tw`flex-1`}>
					<NavigationContainer theme={NavigatorTheme}>
						{showOnboarding ? <OnboardingNavigator /> : <RootNavigator />}
					</NavigationContainer>
					<StatusBar style="light" />
				</GestureHandlerRootView>
			</SafeAreaProvider>
		);
	}
}
