import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { loggerLink } from '@rspc/client';
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
import {
	LibraryContextProvider,
	getDebugState,
	queryClient,
	rspc,
	useCurrentLibrary,
	useInvalidateQuery
} from '@sd/client';
import { GlobalModals } from './components/modal/GlobalModals';
import { reactNativeLink } from './lib/rspcReactNativeTransport';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

const NavigatorTheme: Theme = {
	...DefaultTheme,
	colors: {
		...DefaultTheme.colors,
		// Default screen background
		background: tw.color('app')
	}
};

function AppContainer() {
	// Enables dark mode, and screen size breakpoints, etc. for tailwind
	useDeviceContext(tw, { withDeviceColorScheme: false });

	useInvalidateQuery();

	const { library } = useCurrentLibrary();
	return (
		<SafeAreaProvider style={tw`bg-app flex-1`}>
			<GestureHandlerRootView style={tw`flex-1`}>
				<MenuProvider>
					<BottomSheetModalProvider>
						<StatusBar style="light" />
						<NavigationContainer theme={NavigatorTheme}>
							{!library ? (
								<OnboardingNavigator />
							) : (
								<>
									<RootNavigator />
									<GlobalModals />
								</>
							)}
						</NavigationContainer>
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

export default function App() {
	useEffect(() => {
		SplashScreen.hideAsync();
	}, []);

	return (
		<rspc.Provider client={client} queryClient={queryClient}>
			<LibraryContextProvider
				onNoLibrary={() => {
					console.log('TODO');
				}}
			>
				<AppContainer />
			</LibraryContextProvider>
		</rspc.Provider>
	);
}
