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
import { checkManagePermission, requestManagePermission } from 'manage-external-storage';
import { useEffect, useRef, useState } from 'react';
import { Alert, LogBox, Permission, PermissionsAndroid, Platform } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { MenuProvider } from 'react-native-popup-menu';
import { SafeAreaProvider } from 'react-native-safe-area-context';
import SuperTokens from 'supertokens-react-native';
import { useSnapshot } from 'valtio';
import {
	ClientContextProvider,
	configureAnalyticsProperties,
	LibraryContextProvider,
	P2PContextProvider,
	RspcProvider,
	useBridgeMutation,
	useBridgeQuery,
	useBridgeSubscription,
	useClientContext,
	useInvalidateQuery,
	usePlausibleEvent,
	usePlausiblePageViewMonitor,
	usePlausiblePingMonitor
} from '@sd/client';

import { GlobalModals } from './components/modal/GlobalModals';
import { toast, Toast, toastConfig } from './components/primitive/Toast';
import { useTheme } from './hooks/useTheme';
import { changeTwTheme, tw } from './lib/tailwind';
import RootNavigator from './navigation';
import OnboardingNavigator from './navigation/OnboardingNavigator';
import { P2P } from './screens/p2p/P2P';
import { AUTH_SERVER_URL } from './utils';
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
			configureAnalyticsProperties({ platformType: 'mobile', buildInfo: buildInfo.data });
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
					if (currentRouteName) setCurrentPath(currentRouteName);
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
	const userResponse = useBridgeMutation('cloud.userResponse');

	useBridgeSubscription(['cloud.listenCloudServicesNotifications'], {
		onData: (d) => {
			console.log('Received cloud service notification', d);
			switch (d.kind) {
				case 'ReceivedJoinSyncGroupRequest':
					// WARNING: This is a debug solution to accept the device into the sync group. THIS SHOULD NOT MAKE IT TO PRODUCTION
					userResponse.mutate({
						kind: 'AcceptDeviceInSyncGroup',
						data: {
							ticket: d.data.ticket,
							accepted: {
								id: d.data.sync_group.library.pub_id,
								name: d.data.sync_group.library.name,
								description: null
							}
						}
					});
					// TODO: Move the code above into the dialog below (@Rocky43007)
					// dialogManager.create((dp) => (
					// 	<RequestAddDialog
					// 		device_model={'MacBookPro'}
					// 		device_name={"Arnab's Macbook"}
					// 		library_name={"Arnab's Library"}
					// 		{...dp}
					// 	/>
					// ));
					break;
				default:
					toast.info(`Cloud Service Notification: ${d.kind}`);
					break;
			}
		}
	});

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
		global.Intl = require('intl');
		require('intl/locale-data/jsonp/en'); //TODO(@Rocky43007): Setup a way to import all the languages we support, once we add localization on mobile.
		SuperTokens.init({
			apiDomain: AUTH_SERVER_URL,
			apiBasePath: '/api/auth'
		});
		SplashScreen.hideAsync();
		if (Platform.OS === 'android') {
			(async () => {
				await requestPermissions();
			})();
		}
	}, []);
	return (
		<RspcProvider queryClient={queryClient}>
			<AppContainer />
		</RspcProvider>
	);
}

const requestPermissions = async () => {
	try {
		const granted = await PermissionsAndroid.requestMultiple([
			PermissionsAndroid.PERMISSIONS.READ_MEDIA_AUDIO,
			PermissionsAndroid.PERMISSIONS.READ_MEDIA_IMAGES,
			PermissionsAndroid.PERMISSIONS.READ_MEDIA_VIDEO
		] as Permission[]);

		if (
			granted['android.permission.READ_MEDIA_AUDIO'] === PermissionsAndroid.RESULTS.GRANTED &&
			granted['android.permission.READ_MEDIA_IMAGES'] ===
				PermissionsAndroid.RESULTS.GRANTED &&
			granted['android.permission.READ_MEDIA_VIDEO'] === PermissionsAndroid.RESULTS.GRANTED &&
			PermissionsAndroid.RESULTS.GRANTED
		) {
			const check_MANAGE_EXTERNAL_STORAGE = await checkManagePermission();

			if (!check_MANAGE_EXTERNAL_STORAGE) {
				const request = await requestManagePermission();
				if (!request) {
					Alert.alert(
						'Permission Denied',
						'MANAGE_EXTERNAL_STORAGE permission was denied. The app may not function as expected. Please enable it in the app settings.'
					);
				}
			}
		} else {
			Alert.alert(
				'Permission Denied',
				'Some permissions were denied. The app may not function as expected. Please enable them in the app settings'
			);
		}
	} catch (err) {
		console.warn(err);
	}
};
