import { Linking, Text, View } from 'react-native';
import { useLibraryContext, useLibraryMutation, useLibraryQuery } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { cancel, login, useAuthStateSnapshot } from '~/stores/auth';

const Cloud = ({ navigation }: SettingsStackScreenProps<'Cloud'>) => {
	const authState = useAuthStateSnapshot();

	const authSensitiveChild = () => {
		if (authState.status === 'loggedIn') return <Authenticated />;
		if (authState.status === 'notLoggedIn' || authState.status === 'loggingIn')
			return <Login />;

		return null;
	};

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			{authSensitiveChild()}
		</ScreenContainer>
	);
};

const Authenticated = () => {
	const { library } = useLibraryContext();

	const cloudLibrary = useLibraryQuery(['cloud.library.get'], { suspense: true, retry: false });

	const createLibrary = useLibraryMutation(['cloud.library.create']);
	const syncLibrary = useLibraryMutation(['cloud.library.sync']);

	const thisInstance = cloudLibrary.data?.instances.find(
		(instance) => instance.uuid === library.instance_id
	);

	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			{cloudLibrary.data ? (
				<View style={tw`flex flex-col items-start space-y-2`}>
					<View>
						<Text style={tw`text-ink`}>Library</Text>
						<Text style={tw`text-ink`}>Name: {cloudLibrary.data.name}</Text>
					</View>

					<Button
						disabled={syncLibrary.isLoading}
						onPress={() => {
							syncLibrary.mutateAsync(null);
						}}
					>
						<Text style={tw`text-ink`}>Sync Library</Text>
					</Button>

					{thisInstance && (
						<View>
							<Text style={tw`text-ink`}>This Instance</Text>
							<Text style={tw`text-ink`}>Id: {thisInstance.id}</Text>
							<Text style={tw`text-ink`}>UUID: {thisInstance.uuid}</Text>
							<Text style={tw`text-ink`}>Public Key: {thisInstance.identity}</Text>
						</View>
					)}
					<View>
						<Text style={tw`text-ink`}>Instances</Text>
						<View style={tw`space-y-4 pl-4`}>
							{cloudLibrary.data.instances
								.filter((instance) => instance.uuid !== library.instance_id)
								.map((instance) => (
									<View key={instance.id}>
										<Text style={tw`text-ink`}>Id: {instance.id}</Text>
										<Text style={tw`text-ink`}>UUID: {instance.uuid}</Text>
										<Text style={tw`text-ink`}>Public Key: {instance.identity}</Text>
									</View>
								))}
						</View>
					</View>
				</View>
			) : (
				<View style={tw`relative`}>
					<Button
						disabled={createLibrary.isLoading}
						onPress={() => {
							createLibrary.mutateAsync(null);
						}}
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

const Login = () => {
	const authState = useAuthStateSnapshot();

	return (
		<View style={tw`flex flex-col items-center justify-center gap-2`}>
			<Button
				variant="accent"
				disabled={authState.status === 'loggingIn'}
				onPress={async () => {
					await login();
				}}
			>
				{authState.status !== 'loggingIn' ? <Text style={tw`text-ink`}>Login</Text> : <Text style={tw`text-ink`}>Logging In</Text>}
			</Button>
			{authState.status === 'loggingIn' && (
				<Button
					variant="accent"
					onPress={(e) => {
						e.preventDefault();
						cancel();
					}}
					style={tw`text-sm text-ink-faint`}
				>
					<Text style={tw`text-ink`}>Cancel</Text>
				</Button>
			)}
		</View>
	);
};

export default Cloud;
