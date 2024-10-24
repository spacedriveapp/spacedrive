import { useNavigation } from '@react-navigation/native';
import { Envelope } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import {
	SyncStatus,
	useBridgeMutation,
	useBridgeQuery,
	useLibraryMutation,
	useLibrarySubscription
} from '@sd/client';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { getUserStore, useUserStore } from '~/stores/userStore';
import { AUTH_SERVER_URL, getTokens } from '~/utils';

const AccountProfile = () => {
	const userInfo = useUserStore().userInfo;

	const emailName = userInfo ? userInfo.email.split('@')[0] : '';
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);
	const navigator = useNavigation<SettingsStackScreenProps<'AccountLogin'>['navigation']>();

	const cloudBootstrap = useBridgeMutation('cloud.bootstrap');
	const devices = useBridgeQuery(['cloud.devices.list']);
	const addLibraryToCloud = useLibraryMutation('cloud.libraries.create');
	const listLibraries = useBridgeQuery(['cloud.libraries.list', true]);
	const createSyncGroup = useLibraryMutation('cloud.syncGroups.create');
	const listSyncGroups = useBridgeQuery(['cloud.syncGroups.list']);
	const requestJoinSyncGroup = useBridgeMutation('cloud.syncGroups.request_join');
	const currentDevice = useBridgeQuery(['cloud.devices.get_current_device']);
	const [{ accessToken, refreshToken }, setTokens] = useState<{
		accessToken: string;
		refreshToken: string;
	}>({
		accessToken: '',
		refreshToken: ''
	});
	useEffect(() => {
		(async () => {
			const { accessToken, refreshToken } = await getTokens();
			setTokens({ accessToken, refreshToken });
		})();
	}, []);
	const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
	useLibrarySubscription(['sync.active'], {
		onData: (data) => {
			console.log('sync activity', data);
			setSyncStatus(data);
		}
	});

	async function signOut() {
		await fetch(`${AUTH_SERVER_URL}/api/auth/signout`, {
			method: 'POST'
		});
		navigator.navigate('AccountLogin');
		getUserStore().userInfo = undefined;
	}

	return (
		<ScreenContainer scrollview={false} style={tw`gap-2 px-6`}>
			<View style={tw`flex flex-col justify-between gap-5 lg:flex-row`}>
				<Card
					style={tw`relative flex w-full flex-col items-center justify-center lg:max-w-[320px]`}
				>
					<View style={tw`w-full`}>
						<Text style={tw`mx-auto mt-3 text-lg text-white`}>
							Welcome{' '}
							<Text style={tw`font-bold text-white`}>{capitalizedEmailName}</Text>
						</Text>
						<Card
							style={tw`mt-4 flex-row items-center gap-2 overflow-hidden border-app-inputborder bg-app-input`}
						>
							<Envelope weight="fill" size={20} color="white" />
							<Text numberOfLines={1} style={tw`max-w-[90%] text-white`}>
								{userInfo ? userInfo.email : ''}
							</Text>
						</Card>

						<Button variant="danger" style={tw`mt-3`} onPress={signOut}>
							<Text style={tw`font-bold text-white`}>Sign out</Text>
						</Button>
					</View>
				</Card>
				{/* Sync activity */}
				<View style={tw`mt-5 flex flex-col`}>
					<Text style={tw`mb-2 text-md font-semibold`}>Sync Activity</Text>
					<View style={tw`flex flex-row gap-2`}>
						{Object.keys(syncStatus ?? {}).map((status, index) => (
							<Card key={index} style="flex w-full items-center p-4">
								<View
									style={twStyle(
										'mr-2 size-[15px] rounded-full bg-app-box',
										syncStatus?.[status as keyof SyncStatus]
											? 'bg-accent'
											: 'bg-app-input'
									)}
								/>
								<Text style={tw`text-sm font-semibold`}>{status}</Text>
							</Card>
						))}
					</View>
				</View>

				{/* Automatically list libraries */}
				<View style={tw`mt-5 flex flex-col gap-3`}>
					<Text style={tw`text-md font-semibold text-white`}>Cloud Libraries</Text>
					{listLibraries.data?.map((library) => (
						<Card key={library.pub_id} style={tw`p-41 w-full`}>
							<Text style={tw`text-sm font-semibold text-white`}>{library.name}</Text>
						</Card>
					)) || <Text style={tw`text-white`}>No libraries found.</Text>}
				</View>

				{/* Debug buttons */}
				<Card style={tw`flex gap-2 text-white`}>
					<Button
						variant="gray"
						onPress={async () => {
							cloudBootstrap.mutate([accessToken.trim(), refreshToken.trim()]);
						}}
					>
						<Text style={tw`text-white`}>Start Cloud Bootstrap</Text>
					</Button>
					<Button
						variant="gray"
						onPress={async () => {
							addLibraryToCloud.mutate(null);
						}}
					>
						<Text style={tw`text-white`}>Add Library to Cloud</Text>
					</Button>
					<Button
						variant="gray"
						onPress={async () => {
							createSyncGroup.mutate(null);
						}}
					>
						<Text style={tw`text-white`}>Create Sync Group</Text>
					</Button>
				</Card>

				<View style={tw`mt-5 flex flex-col gap-3 text-white`}>
					<Text style={tw`text-md font-semibold`}>Library Sync Groups</Text>
					{listSyncGroups.data?.map((group) => (
						<Card key={group.pub_id} style="w-full p-4">
							<Text style={tw`text-sm font-semibold text-white`}>
								{group.library.name}
							</Text>
							<Button
								style={tw`mt-2`}
								onPress={async () => {
									if (!currentDevice.data) await currentDevice.refetch();
									if (currentDevice.data && devices.data) {
										requestJoinSyncGroup.mutate({
											asking_device: currentDevice.data,
											sync_group: {
												devices: devices.data,
												...group
											}
										});
									}
								}}
							>
								<Text style={tw`text-white`}>Join Sync Group</Text>
							</Button>
						</Card>
					)) || <Text style={tw`text-white`}>No sync groups found.</Text>}
				</View>
			</View>
		</ScreenContainer>
	);
};

export default AccountProfile;
