import { Text, View } from 'react-native';
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { cancel, login, useAuthStateSnapshot } from '~/stores/auth';

const Login = () => {
	const authState = useAuthStateSnapshot();
	const buttonText = {
		notLoggedIn: 'Login',
		loggingIn: 'Cancel'
	};
	return (
		<View style={tw`flex-1 flex-col items-center justify-center gap-2`}>
			<Card style={tw`w-full items-center justify-center py-6`}>
				<View style={tw`flex-col items-center gap-2`}>
				<Icon name="CloudSync" size={64} />
				<Text style={tw`mb-4 max-w-[60%] text-center text-ink`}>
					To access cloud related features, please login
				</Text>
				</View>
				{(authState.status === 'notLoggedIn' || authState.status === 'loggingIn') && (
					<Button
						variant="accent"
						style={tw`mx-auto mt-1 max-w-[50%]`}
						onPress={async (e) => {
							e.preventDefault();
							if (authState.status === 'loggingIn') {
								await cancel();
							} else {
								await login();
							}
						}}
					>
						<Text style={tw`font-medium text-ink`}>{buttonText[authState.status]}</Text>
					</Button>
				)}
			</Card>
		</View>
	);
};

export default Login;
