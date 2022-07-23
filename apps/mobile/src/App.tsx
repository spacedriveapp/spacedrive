import { NavigationContainer } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import React from 'react';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';

import useCachedResources from './hooks/useCachedResources';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';

export default function App() {
	// Enables dark mode, and screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	const isLoadingComplete = useCachedResources();

	// TODO: Show onboarding navigator if first time.

	if (!isLoadingComplete) {
		return null;
	} else {
		return (
			<SafeAreaProvider>
				<NavigationContainer>
					<OnboardingNavigator />
					{/* <RootNavigator /> */}
				</NavigationContainer>
				<StatusBar />
			</SafeAreaProvider>
		);
	}
}
