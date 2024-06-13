import { CloudInstance, useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { CheckCircle } from 'phosphor-react-native';
import { useMemo } from 'react';
import { ActivityIndicator, FlatList, Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import Empty from '~/components/layout/Empty';
import ScreenContainer from '~/components/layout/ScreenContainer';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { styled, tw, twStyle } from '~/lib/tailwind';
import { cancel, login, logout, useAuthStateSnapshot } from '~/stores/auth';

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
	if (authState.status === 'notLoggedIn' || authState.status === 'loggingIn') return <Login />;

	return null;
};

const Authenticated = () => {
	const { library } = useLibraryContext();
	const authState = useAuthStateSnapshot();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);

	const thisInstance = useMemo(() => cloudLibrary.data?.instances.find(
		(instance) => instance.uuid === library.instance_id
	), [cloudLibrary.data, library.instance_id]);

	const cloudInstances = useMemo(() =>
		cloudLibrary.data?.instances.filter(
			(instance) => instance.uuid !== library.instance_id
	), [cloudLibrary.data, library.instance_id]);

	const isLibrarySynced = useMemo(() =>
		cloudLibrary.data?.instances.some(
			(instance) => instance.uuid === library.instance_id
	),[cloudLibrary.data, library]);

	if (cloudLibrary.isLoading) {
		return (
			<View style={tw`flex-1 items-center justify-center`}>
				<ActivityIndicator size="small"/>
			</View>
		);
	}

	return (
		<ScreenContainer  tabHeight={false}>
			{cloudLibrary.data ? (
				<View style={tw`flex-col items-start gap-5`}>
					<Card style={tw`w-full`}>
					<View style={tw`flex-row items-center justify-between`}>
					<Text style={tw`font-medium text-ink`}>Library</Text>
					{authState.status === 'loggedIn' && (
						<Button
						variant="gray"
						size="sm"
						onPress={logout}
						>
						<Text style={tw`text-xs font-semibold text-ink`}>Logout</Text>
					</Button>
					)}
					</View>
					<Divider style={tw`mb-4 mt-2`}/>
						<SettingsTitle style={tw`mb-2`}>Name</SettingsTitle>
						<InfoBox>
							<Text style={tw`text-ink`}>{cloudLibrary.data.name}</Text>
						</InfoBox>
						<Button
							disabled={syncLibrary.isLoading}
							variant={isLibrarySynced ? 'dashed' : 'accent'}
							style={tw`mt-2 flex-row gap-1 py-2`}
							onPress={() => syncLibrary.mutateAsync(null)}
						>
							{isLibrarySynced && <CheckCircle size={13} weight="fill" color={tw.color('green-500')}/>}
							<Text style={tw`text-xs font-semibold text-ink`}>{
								isLibrarySynced
								? 'Library synced'
								: 'Sync library'
							}</Text>
						</Button>
					</Card>
					{thisInstance && (
						<Card style={tw`w-full gap-4`}>
							<View>
							<Text style={tw`mb-1 font-semibold text-ink`}>This Instance</Text>
							<Divider />
							</View>
							<View>
							<SettingsTitle style={tw`mb-2 text-ink`}>Id</SettingsTitle>
							<InfoBox>
								<Text style={tw`text-ink-dull`}>{thisInstance.id}</Text>
							</InfoBox>
							</View>
							<View>
							<SettingsTitle style={tw`mb-2`}>UUID</SettingsTitle>
							<InfoBox>
								<Text style={tw`text-ink-dull`}>{thisInstance.uuid}</Text>
							</InfoBox>
							</View>
							<View>
							<SettingsTitle style={tw`mb-2`}>Public Key</SettingsTitle>
							<InfoBox>
								<Text numberOfLines={1} style={tw`text-ink-dull`}>
									{thisInstance.identity}
								</Text>
							</InfoBox>
							</View>
						</Card>
					)}
					<Card style={tw`w-full`}>
						<View style={tw`flex-row items-center gap-2`}>
							<View
								style={tw`self-start rounded border border-app-lightborder bg-app-highlight px-1.5 py-[2px]`}
							>
								<Text style={tw`text-xs font-semibold text-ink`}>
									{cloudInstances?.length}
								</Text>
							</View>
							<Text style={tw`font-semibold text-ink`}>Instances</Text>
						</View>
						<Divider style={tw`mb-4 mt-2`} />
						<VirtualizedListWrapper
							scrollEnabled={false}
							contentContainerStyle={tw`flex-1`}
							horizontal
						>
							<FlatList
								data={cloudInstances}
								scrollEnabled={false}
								ListEmptyComponent={<Empty
									textStyle={tw`my-0`}
									description='No instances found'
								/>}
								contentContainerStyle={twStyle(cloudInstances?.length === 0 && 'flex-row')}
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
				<Card style={tw`relative py-10`}>
					<Button
					style={tw`mx-auto max-w-[82%]`}
					disabled={createLibrary.isLoading}
					onPress={async () => await createLibrary.mutateAsync(null)}
					>
						{createLibrary.isLoading ? (
							<Text style={tw`text-ink`}>
								Connecting library to Spacedrive Cloud...
							</Text>
						) : (
							<Text style={tw`font-medium text-ink`}>Connect library to Spacedrive Cloud</Text>
						)}
					</Button>
				</Card>
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
		<InfoBox style={twStyle(length > 1 ? 'w-[49%]' : 'w-full', 'gap-4')}>
				<View>
				<SettingsTitle style={tw`mb-2`}>Id</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.id}</Text>
				</InfoBox>
				</View>
				<View>
				<SettingsTitle style={tw`mb-2`}>UUID</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.uuid}</Text>
				</InfoBox>
				</View>
				<View>
				<SettingsTitle style={tw`mb-2`}>Public Key</SettingsTitle>
				<InfoBox>
					<Text numberOfLines={1} style={tw`text-ink-dull`}>{data.identity}</Text>
				</InfoBox>
				</View>
		</InfoBox>
	);
};

const Login = () => {
	const authState = useAuthStateSnapshot();
	const buttonText = {
		notLoggedIn: 'Login',
		loggingIn: 'Cancel',
	}
	return (
		<View style={tw`flex-1 flex-col items-center justify-center gap-2`}>
			<Card style={tw`w-full items-center justify-center p-6`}>
				<Text style={tw`mb-4 max-w-[60%] text-center text-ink`}>
					To access cloud related features, please login
				</Text>
			{(authState.status === 'notLoggedIn' || authState.status === 'loggingIn') && (
					<Button
					variant="accent"
					style={tw`mx-auto max-w-[50%]`}
					onPress={async (e) => {
						e.preventDefault();
						if (authState.status === 'loggingIn') {
							await cancel();
						} else {
							await login();
						}
					}}
				>
					<Text style={tw`font-medium text-ink`}>
						{buttonText[authState.status]}
					</Text>
				</Button>
			)}
			</Card>
		</View>
	);
};

export default CloudSettings;
