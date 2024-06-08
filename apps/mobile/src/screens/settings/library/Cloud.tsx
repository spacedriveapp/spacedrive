import { Linking, Text, View } from 'react-native';
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
	return (
		<ScreenContainer scrollview={false} style={tw`gap-0 px-6`}>
			<Text style={tw`text-ink`}>You are authenticated!</Text>
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
				{authState.status !== 'loggingIn' ? <Text>Login</Text> : <Text>Logging In</Text>}
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
					<Text>Cancel</Text>
				</Button>
			)}
		</View>
	);
};

export default Cloud;
