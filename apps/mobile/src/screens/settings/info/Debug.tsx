import React from 'react';
import { Text, View } from 'react-native';
import {
	useBridgeMutation,
	useBridgeQuery,
	useDebugState,
	useFeatureFlags,
	useLibraryMutation
} from '@sd/client';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { getTokens } from '~/utils';

const DebugScreen = ({ navigation }: SettingsStackScreenProps<'Debug'>) => {
	const debugState = useDebugState();
	const featureFlags = useFeatureFlags();
	const [tokens, setTokens] = React.useState({ accessToken: '', refreshToken: '' });
	const accessToken = tokens.accessToken;
	const refreshToken = tokens.refreshToken;
	// const origin = useBridgeQuery(['cloud.getApiOrigin']);
	// const setOrigin = useBridgeMutation(['cloud.setApiOrigin']);

	React.useEffect(() => {
		async function _() {
			const _a = await getTokens();
			setTokens({ accessToken: _a.accessToken, refreshToken: _a.refreshToken });
		}
		_();
	}, []);

	const cloudBootstrap = useBridgeMutation(['cloud.bootstrap']);
	const addLibraryToCloud = useLibraryMutation('cloud.libraries.create');
	const requestJoinSyncGroup = useBridgeMutation('cloud.syncGroups.request_join');
	const getGroup = useBridgeQuery([
		'cloud.syncGroups.get',
		{
			pub_id: '01924497-a1be-76e3-b62f-9582ea15463a',
			// pub_id: '01924a25-966b-7c00-a582-9eed3aadd2cd',
			kind: 'WithDevices'
		}
	]);
	// console.log(getGroup.data);
	const currentDevice = useBridgeQuery(['cloud.devices.get_current_device']);
	// console.log('Current Device: ', currentDevice.data);
	const createSyncGroup = useLibraryMutation('cloud.syncGroups.create');

	// const queryClient = useQueryClient();

	return (
		<View style={tw`flex-1 p-4`}>
			<Card style={tw`gap-y-4`}>
				<Text style={tw`font-semibold text-ink`}>Debug</Text>
				<Button onPress={() => (debugState.rspcLogger = !debugState.rspcLogger)}>
					<Text style={tw`text-ink`}>Toggle rspc logger</Text>
				</Button>
				<Text style={tw`text-ink`}>{JSON.stringify(featureFlags)}</Text>
				<Text style={tw`text-ink`}>{JSON.stringify(debugState)}</Text>
				{/* <Button
					onPress={() => {
						navigation.popToTop();
						navigation.replace('Settings');
						debugState.enabled = false;
					}}
				>
					<Text style={tw`text-ink`}>Disable Debug Mode</Text>
				</Button> */}
				{/* <Button
					onPress={() => {
						const url =
							origin.data === 'https://api.spacedrive.com'
								? 'http://localhost:3000'
								: 'https://api.spacedrive.com';
						setOrigin.mutateAsync(url).then(async () => {
							await auth.logout();
							await queryClient.invalidateQueries();
						});
					}}
				>
					<Text style={tw`text-ink`}>Toggle API Route ({origin.data})</Text>
				</Button> */}
				<Button
					onPress={() => {
						navigation.popToTop();
						navigation.navigate('BackfillWaitingStack', {
							screen: 'BackfillWaiting'
						});
					}}
				>
					<Text style={tw`text-ink`}>Go to Backfill Waiting Page</Text>
				</Button>
				{/* <Button
					onPress={async () => {
						await auth.logout();
					}}
				>
					<Text style={tw`text-ink`}>Logout</Text>
				</Button> */}
				<Button
					onPress={async () => {
						const tokens = await getTokens();
						cloudBootstrap.mutate([tokens.accessToken, tokens.refreshToken]);
					}}
				>
					<Text style={tw`text-ink`}>Cloud Bootstrap</Text>
				</Button>
				<Button
					onPress={async () => {
						addLibraryToCloud.mutate(null);
					}}
				>
					<Text style={tw`text-ink`}>Add Library to Cloud</Text>
				</Button>
				<Button
					onPress={async () => {
						createSyncGroup.mutate(null);
					}}
				>
					<Text style={tw`text-ink`}>Create Sync Group</Text>
				</Button>
				<Button
					onPress={async () => {
						if (
							currentDevice.data &&
							getGroup.data &&
							getGroup.data.kind === 'WithDevices'
						) {
							currentDevice.refetch();
							console.log('Current Device: ', currentDevice.data);
							console.log('Get Group: ', getGroup.data.data);
							requestJoinSyncGroup.mutate({
								sync_group: getGroup.data.data,
								asking_device: currentDevice.data
							});
						}
					}}
				>
					<Text style={tw`text-ink`}>Request Join Sync Group</Text>
				</Button>
			</Card>
		</View>
	);
};

export default DebugScreen;
