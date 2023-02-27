import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { loggerLink } from '@rspc/client';
import { QueryClient } from "@tanstack/react-query";
import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { useEffect } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { MenuProvider } from 'react-native-popup-menu';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';
import { proxy, useSnapshot } from 'valtio';
import {
	ClientContextProvider,
	LibraryContextProvider,
	getDebugState,
	rspc,
	useClientContext,
	useInvalidateQuery
} from '@sd/client';
import { GlobalModals } from './components/modal/GlobalModals';
import { reactNativeLink } from './lib/rspcReactNativeTransport';
import { tw } from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { currentLibraryStore } from './utils/nav';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

const NavigatorTheme: Theme = {
	...DefaultTheme,
	colors: {
		...DefaultTheme.colors,
		// Default screen background
		background: tw.color('app')!
	}
};

function AppNavigation() {
	const { library } = useClientContext();

	// TODO: Make sure library has actually been loaded by this point - precache with useCachedLibraries?
	// if (library === undefined) throw new Error("Tried to render AppNavigation before libraries fetched!")

	return (
		<NavigationContainer theme={NavigatorTheme}>
			{!library ? (
				<OnboardingNavigator />
			) : (
				<LibraryContextProvider library={library}>
					<RootNavigator />
					<GlobalModals />
				</LibraryContextProvider>
			)}
		</NavigationContainer>
	);
}

function AppContainer() {
	// Enables dark mode, and screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	useInvalidateQuery();

	const { id } = useSnapshot(currentLibraryStore);

	return (
		<SafeAreaProvider style={tw`bg-app flex-1`}>
			<GestureHandlerRootView style={tw`flex-1`}>
				<MenuProvider>
					<BottomSheetModalProvider>
						<StatusBar style="light" />
						<ClientContextProvider currentLibraryId={id}>
							<AppNavigation />
						</ClientContextProvider>
					</BottomSheetModalProvider>
				</MenuProvider>
			</GestureHandlerRootView>
		</SafeAreaProvider>
	);
}

const client = rspc.createClient({
	links: [
		loggerLink({
			enabled: () => getDebugState().rspcLogger
		}),
		reactNativeLink()
	]
});

const queryClient = new QueryClient();

export default function App() {
	useEffect(() => {
		SplashScreen.hideAsync();
	}, []);

	return (
		<rspc.Provider client={client} queryClient={queryClient}>
			<AppContainer />
		</rspc.Provider>
	);
}
