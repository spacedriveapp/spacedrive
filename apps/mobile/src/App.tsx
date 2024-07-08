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
import { Alert, LogBox, Platform } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import {
	check,
	Permission,
	PERMISSIONS,
	PermissionStatus,
	request,
	RESULTS
} from 'react-native-permissions';
import { MenuProvider } from 'react-native-popup-menu';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import { useSnapshot } from 'valtio';
import {
	ClientContextProvider,
	initPlausible,
	LibraryContextProvider,
	P2PContextProvider,
	RspcProvider,
	useBridgeQuery,
	useClientContext,
	useInvalidateQuery,
	usePlausibleEvent,
	usePlausiblePageViewMonitor,
	usePlausiblePingMonitor
} from '@sd/client';

import { GlobalModals } from './components/modal/GlobalModals';
import { Toast, toastConfig } from './components/primitive/Toast';
import { useTheme } from './hooks/useTheme';
import { changeTwTheme, tw } from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { P2P } from './screens/p2p/P2P';
import { currentLibraryStore } from './utils/nav';

LogBox.ignoreLogs(['Sending `onAnimatedValueUpdate` with no listeners registered.']);

dayjs.extend(advancedFormat);
dayjs.extend(relativeTime);
dayjs.extend(duration);

// changeTwTheme(getThemeStore().theme);
// TODO: Use above when light theme is ready
changeTwTheme('dark');

function AppNavigation() {
	const { libraries, library } = useClientContext();
	const plausibleEvent = usePlausibleEvent();
	const buildInfo = useBridgeQuery(['buildInfo']);

	const navRef = useNavigationContainerRef();
	const routeNameRef = useRef<string>();

	const [currentPath, setCurrentPath] = useState<string>('/');

	useEffect(() => {
		if (buildInfo?.data) {
			initPlausible({ platformType: 'mobile', buildInfo: buildInfo.data });
		}
	}, [buildInfo]);

	usePlausiblePageViewMonitor({ currentPath });
	usePlausiblePingMonitor({ currentPath });

	useEffect(() => {
		const interval = setInterval(() => {
			plausibleEvent({ event: { type: 'ping' } });
		}, 270 * 1000);

		return () => clearInterval(interval);
	}, [plausibleEvent]);

	useEffect(() => {
		if (library === null && libraries.data) {
			currentLibraryStore.id = libraries.data[0]?.uuid ?? null;
		}
	}, [library, libraries]);

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
					background: 'black'
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
		<SafeAreaProvider style={tw`flex-1 bg-black`}>
			<GestureHandlerRootView style={tw`flex-1`}>
				<MenuProvider>
					<BottomSheetModalProvider>
						<StatusBar style="light" />
						<ClientContextProvider currentLibraryId={id}>
							<P2PContextProvider>
								<P2P />
								<AppNavigation />
								<Toast config={toastConfig} />
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
		if (Platform.OS === 'android') checkAndroidPermissions();
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
			<AppContainer />
		</RspcProvider>
	);
}

const checkAndroidPermissions = async () => {
	try {
		const permissions = [
			PERMISSIONS.ANDROID.READ_MEDIA_AUDIO,
			PERMISSIONS.ANDROID.READ_MEDIA_IMAGES,
			PERMISSIONS.ANDROID.READ_MEDIA_VIDEO,
			// PERMISSIONS.ANDROID.ACCESS_MEDIA_LOCATION, // Not used/Needed right now but may be needed in the future
			PERMISSIONS.ANDROID.READ_EXTERNAL_STORAGE,
			PERMISSIONS.ANDROID.WRITE_EXTERNAL_STORAGE
		];

		let allPermissionsGranted = false;
		const blockedPermissions = [];

		while (!allPermissionsGranted) {
			const statuses = await Promise.all(permissions.map((permission) => check(permission)));

			const deniedPermissions = statuses
				.map((status, index) => (status === RESULTS.DENIED ? permissions[index] : null))
				.filter((permission) => permission !== null);

			for (const permission of deniedPermissions) {
				const result = await request(permission as Permission);
				if (result === RESULTS.BLOCKED) {
					blockedPermissions.push(permission);
				}
			}

			allPermissionsGranted = statuses.every((status) => status === RESULTS.GRANTED);

			if (!allPermissionsGranted && blockedPermissions.length > 0) {
				Alert.alert(
					'Permissions required',
					'Some permissions are blocked. Please grant the permissions in Settings.',
					[{ text: 'OK' }]
				);
				break;
			}
			console.log('statuses', statuses);
		}

		if (!allPermissionsGranted) {
			Alert.alert(
				'Permissions required',
				'Some permissions are not granted. The app may not function correctly.'
			);
		} else {
			Alert.alert('Permissions granted', 'All required permissions have been granted.');
		}
	} catch (err) {
		console.warn(err);
	}
};
