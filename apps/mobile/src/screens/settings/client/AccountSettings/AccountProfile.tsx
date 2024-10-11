import { useNavigation } from '@react-navigation/native';
import { Envelope } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { getUserStore, useUserStore } from '~/stores/userStore';
import { AUTH_SERVER_URL } from '~/utils';

const AccountProfile = () => {
	const userInfo = useUserStore().userInfo;

	const emailName = userInfo ? userInfo.email.split('@')[0] : '';
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);
	const navigator = useNavigation<SettingsStackScreenProps<'AccountLogin'>['navigation']>();

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
			</View>
		</ScreenContainer>
	);
};

export default AccountProfile;
