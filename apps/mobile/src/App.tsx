import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import {
	DefaultTheme,
	NavigationContainer,
	Theme,
	useNavigationContainerRef
} from '@react-navigation/native';
import { loggerLink } from '@rspc/client';
import { QueryClient } from '@tanstack/react-query';
import dayjs from 'dayjs';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import duration from 'dayjs/plugin/duration';
import relativeTime from 'dayjs/plugin/relativeTime';
import * as SplashScreen from 'expo-splash-screen';
import { StatusBar } from 'expo-status-bar';
import { useEffect, useRef, useState } from 'react';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { MenuProvider } from 'react-native-popup-menu';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useDeviceContext } from 'twrnc';
import { useSnapshot } from 'valtio';
import {
	ClientContextProvider,
	LibraryContextProvider,
	getDebugState,
	initPlausible,
	rspc,
	useClientContext,
	useInvalidateQuery,
	usePlausiblePageViewMonitor
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

initPlausible({ platformType: 'mobile' });

function AppNavigation() {
	const { library } = useClientContext();

	// TODO: Make sure library has actually been loaded by this point - precache with useCachedLibraries?
	// if (library === undefined) throw new Error("Tried to render AppNavigation before libraries fetched!")

	const navRef = useNavigationContainerRef();
	const routeNameRef = useRef<string>();

	const [currentPath, setCurrentPath] = useState<string>('/');

	usePlausiblePageViewMonitor({ currentPath });

	return (
		<NavigationContainer
			ref={navRef}
			onReady={() => {
				routeNameRef.current = navRef.getCurrentRoute()?.name;
			}}
			theme={NavigatorTheme}
			onStateChange={async () => {
				const previousRouteName = routeNameRef.current;
				const currentRouteName = navRef.getCurrentRoute()?.name;
				if (previousRouteName !== currentRouteName) {
					// Save the current route name for later comparison
					routeNameRef.current = currentRouteName;
					// Don't track onboarding screens
					if (navRef.getRootState().routeNames.includes('GetStarted')) {
						return;
					}
					console.log(`Navigated from ${previousRouteName} to ${currentRouteName}`);
					currentRouteName && setCurrentPath(currentRouteName);
				}
			}}
		>
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
		// @ts-expect-error: Version mismatch
		<rspc.Provider client={client} queryClient={queryClient}>
			<AppContainer />
		</rspc.Provider>
	);
}
