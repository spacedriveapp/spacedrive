import { CloudInstance, useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { FlatList, Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { styled, tw, twStyle } from '~/lib/tailwind';
import { cancel, login, useAuthStateSnapshot } from '~/stores/auth';

const InfoBox = styled(View, 'rounded-md border border-app bg-transparent p-2');

const CloudSettings = () => {
	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6 py-0`}>
			<AuthSensitiveChild />
		</ScreenContainer>
	);
};

const AuthSensitiveChild = () => {
	const authState = useAuthStateSnapshot();
	if (authState.status === 'loggedIn') return <Authenticated />;
	if (authState.status === 'notLoggedIn' || authState.status === 'loggingIn')
		return <Login />;

	return null;
};

const Authenticated = () => {
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);

	const thisInstance = cloudLibrary.data?.instances.find(
		(instance) => instance.uuid === library.instance_id
	);
	const cloudInstances = cloudLibrary.data?.instances
	.filter((instance) => instance.uuid !== library.instance_id)

	return (
		<ScreenContainer tabHeight={false}>
			{cloudLibrary.data ? (
				<View style={tw`flex-col items-start gap-5`}>
					<Card style={tw`w-full`}>
					<Text style={tw`font-semibold text-ink`}>Library</Text>
					<Divider style={tw`mb-4 mt-2`}/>
						<SettingsTitle style={tw`mb-1`}>Name</SettingsTitle>
						<InfoBox>
							<Text style={tw`text-ink`}>{cloudLibrary.data.name}</Text>
						</InfoBox>
					<Button
						disabled={syncLibrary.isLoading}
						variant="accent"
						style={tw`mt-2`}
						onPress={() => {
							syncLibrary.mutateAsync(null);
						}}
					>
						<Text style={tw`text-xs font-medium text-ink`}>Sync Library</Text>
					</Button>
					</Card>
					{thisInstance && (
						<Card style={tw`w-full`}>
							<Text style={tw`font-semibold text-ink`}>This Instance</Text>
							<Divider style={tw`mb-4 mt-2`}/>
							<SettingsTitle style={tw`mb-1 text-ink`}>Id</SettingsTitle>
							<InfoBox>

								<Text style={tw`text-ink-dull`}>{thisInstance.id}</Text>
							</InfoBox>
							<SettingsTitle style={tw`mb-1 mt-4`}>UUID</SettingsTitle>
							<InfoBox>
								<Text style={tw`text-ink-dull`}>{thisInstance.uuid}</Text>
							</InfoBox>
							<SettingsTitle style={tw`mb-1 mt-4`}>Public Key</SettingsTitle>
							<InfoBox>
								<Text numberOfLines={1} style={tw`text-ink-dull`}>{thisInstance.identity}</Text>
							</InfoBox>
						</Card>
					)}
					<Card style={tw`w-full`}>
						<View style={tw`flex-row items-center gap-2`}>
						<View
							style={tw`self-start rounded border border-app-lightborder bg-app-highlight px-1.5 py-[2px]`}
						>
							<Text style={tw`text-xs font-semibold text-ink`}>{cloudInstances?.length}</Text>
						</View>
						<Text style={tw`font-semibold text-ink`}>Instances</Text>
						</View>
						<Divider style={tw`mb-4 mt-2`}/>
						<VirtualizedListWrapper scrollEnabled={false} contentContainerStyle={tw`flex-1`} horizontal>
							<FlatList
								data={cloudInstances}
								scrollEnabled={false}
								showsHorizontalScrollIndicator={false}
								ItemSeparatorComponent={() => <View style={tw`h-2`}/>}
								renderItem={({ item }) => <Instance data={item} length={cloudInstances?.length ?? 0} />}
								keyExtractor={(item) => item.id}
								numColumns={(cloudInstances?.length ?? 0) > 1 ? 2 : 1}
								{...(cloudInstances?.length ?? 0) > 1 ? {columnWrapperStyle: tw`w-full justify-between`} : {}}
								/>
						</VirtualizedListWrapper>
					</Card>
				</View>
			) : (
				<View style={tw`relative`}>
					<Button
						disabled={createLibrary.isLoading}
						onPress={async () => await createLibrary.mutateAsync(null)}
					>
						{createLibrary.isLoading ? (
							<Text style={tw`text-ink`}>Connecting library to Spacedrive Cloud...</Text>
						) : (
							<Text style={tw`text-ink`}>Connect library to Spacedrive Cloud</Text>
						)}
					</Button>
				</View>
			)}
		</ScreenContainer>
	);
};

interface Props {
	data: CloudInstance;
	length: number;
}

const Instance = ({data, length}: Props) => {
	return (
		<InfoBox style={twStyle(length > 1 ? 'w-[49%]' : 'w-full')}>
				<SettingsTitle style={tw`mb-1`}>Id</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.id}</Text>
				</InfoBox>
				<SettingsTitle style={tw`mb-1 mt-4`}>UUID</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.uuid}</Text>
				</InfoBox>
				<SettingsTitle style={tw`mb-1 mt-4`}>Public Key</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.identity}</Text>
				</InfoBox>
		</InfoBox>
	)
}

const Login = () => {
	const authState = useAuthStateSnapshot();
	return (
		<View style={tw`flex flex-col items-center justify-center gap-2`}>
			<Button
				variant="accent"
				disabled={authState.status === 'loggingIn'}
				onPress={async (e) => {
					e.preventDefault();
					await login();
				}}
			>
				{authState.status !== 'loggingIn' ? <Text style={tw`text-ink`}>Login</Text> : <Text style={tw`text-ink`}>Logging In</Text>}
			</Button>
			{authState.status === 'loggingIn' && (
				<Button
					variant="accent"
					onPress={async (e) => {
						e.preventDefault();
						await cancel();
					}}
					style={tw`text-sm text-ink-faint`}
				>
					<Text style={tw`text-ink`}>Cancel</Text>
				</Button>
			)}
		</View>
	);
};

export default CloudSettings;
