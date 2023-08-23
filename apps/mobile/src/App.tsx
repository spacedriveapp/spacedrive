import { BottomSheetModalProvider } from '@gorhom/bottom-sheet';
import {
	DefaultTheme,
	NavigationContainer,
	useNavigationContainerRef
} from '@react-navigation/native';
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
import { useSnapshot } from 'valtio';
import {
	ClientContextProvider,
	LibraryContextProvider,
	NotificationContextProvider,
	P2PContextProvider,
	RspcProvider,
	initPlausible,
	useClientContext,
	useInvalidateQuery,
	usePlausibleEvent,
	usePlausiblePageViewMonitor,
	usePlausiblePingMonitor
} from '@sd/client';
import { GlobalModals } from './components/modal/GlobalModals';
import { useTheme } from './hooks/useTheme';
import { changeTwTheme, tw } from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { P2P } from './screens/p2p';
import { currentLibraryStore } from './utils/nav';

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

initPlausible({ platformType: 'mobile' });
// changeTwTheme(getThemeStore().theme);
// TODO: Use above when light theme is ready
changeTwTheme('dark');

function AppNavigation() {
	const { libraries, library } = useClientContext();
	const plausibleEvent = usePlausibleEvent();

	// TODO: Make sure library has actually been loaded by this point - precache with useCachedLibraries?
	// if (library === undefined) throw new Error("Tried to render AppNavigation before libraries fetched!")

	const navRef = useNavigationContainerRef();
	const routeNameRef = useRef<string>();

	const [currentPath, setCurrentPath] = useState<string>('/');

	usePlausiblePageViewMonitor({ currentPath });
	usePlausiblePingMonitor({ currentPath });

	useEffect(() => {
		const interval = setInterval(() => {
			plausibleEvent({
				event: {
					type: 'ping'
				}
			});
		}, 270 * 1000);

		return () => clearInterval(interval);
	}, []);

	if (library === null && libraries.data) {
		currentLibraryStore.id = libraries.data[0]?.uuid ?? null;
	}

	return (
		<NavigationContainer
			ref={navRef}
			onReady={() => {
				routeNameRef.current = navRef.getCurrentRoute()?.name;
			}}
			theme={{
				...DefaultTheme,
				colors: {
					...DefaultTheme.colors,
					// Default screen background
					background: tw.color('app')!
				}
			}}
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
	useTheme();
	useInvalidateQuery();

	const { id } = useSnapshot(currentLibraryStore);

	return (
		<SafeAreaProvider style={tw`flex-1 bg-app`}>
			<GestureHandlerRootView style={tw`flex-1`}>
				<MenuProvider>
					<BottomSheetModalProvider>
						<StatusBar style="light" />
						<ClientContextProvider currentLibraryId={id}>
							<P2PContextProvider>
								<P2P />
								<NotificationContextProvider>
									<AppNavigation />
								</NotificationContextProvider>
							</P2PContextProvider>
						</ClientContextProvider>
					</BottomSheetModalProvider>
				</MenuProvider>
			</GestureHandlerRootView>
		</SafeAreaProvider>
	);
}

const queryClient = new QueryClient();

export default function App() {
	useEffect(() => {
		SplashScreen.hideAsync();
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
			<AppContainer />
		</RspcProvider>
	);
}
