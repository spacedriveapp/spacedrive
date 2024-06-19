import { useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { useMemo } from 'react';
import { ActivityIndicator, FlatList, Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import Empty from '~/components/layout/Empty';
import ScreenContainer from '~/components/layout/ScreenContainer';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { styled, tw, twStyle } from '~/lib/tailwind';
import { useAuthStateSnapshot } from '~/stores/auth';

import Instance from './Instance';
import Library from './Library';
import Login from './Login';
import ThisInstance from './ThisInstance';

export const InfoBox = styled(View, 'rounded-md border gap-1 border-app bg-transparent p-2');

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
	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { retry: false });
	const createLibrary = useLibraryMutation(['cloud.library.create']);

	const cloudInstances = useMemo(
		() =>
			cloudLibrary.data?.instances.filter(
				(instance) => instance.uuid !== library.instance_id
			),
		[cloudLibrary.data, library.instance_id]
	);

	if (cloudLibrary.isLoading) {
		return (
			<View style={tw`flex-1 items-center justify-center`}>
				<ActivityIndicator size="small" />
			</View>
		);
	}

	return (
		<ScreenContainer tabHeight={false}>
			{cloudLibrary.data ? (
				<View style={tw`flex-col items-start gap-5`}>
					<Library cloudLibrary={cloudLibrary.data} />
					<ThisInstance cloudLibrary={cloudLibrary.data} />
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
								ListEmptyComponent={
									<Empty textStyle={tw`my-0`} description="No instances found" />
								}
								contentContainerStyle={twStyle(
									cloudInstances?.length === 0 && 'flex-row'
								)}
								showsHorizontalScrollIndicator={false}
								ItemSeparatorComponent={() => <View style={tw`h-2`} />}
								renderItem={({ item }) => (
									<Instance data={item} />
								)}
								keyExtractor={(item) => item.id}
								numColumns={1}
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
							<Text style={tw`font-medium text-ink`}>
								Connect library to Spacedrive Cloud
							</Text>
						)}
					</Button>
				</Card>
			)}
		</ScreenContainer>
	);
};

export default CloudSettings;
