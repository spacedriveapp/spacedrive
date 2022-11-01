import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import { DefaultTheme, NavigationContainer, Theme } from '@react-navigation/native';
import { loggerLink } from '@rspc/client';
import {
	LibraryContextProvider,
	getDebugState,
	queryClient,
	rspc,
	useCurrentLibrary,
	useInvalidateQuery
} from '@sd/client';
import { StatusBar } from 'expo-status-bar';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';

import { GlobalModals } from './containers/modals/GlobalModals';
import { reactNativeLink } from './lib/rspcReactNativeTransport';
import tw from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';

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

	useInvalidateQuery();

	const { library } = useCurrentLibrary();
	return (
		<SafeAreaProvider style={tw`flex-1 bg-gray-650`}>
			<GestureHandlerRootView style={tw`flex-1`}>
				<BottomSheetModalProvider>
					<StatusBar style="light" />
					<NavigationContainer theme={NavigatorTheme}>
						{!library ? <OnboardingNavigator /> : <RootNavigator />}
					</NavigationContainer>
					<GlobalModals />
				</BottomSheetModalProvider>
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
