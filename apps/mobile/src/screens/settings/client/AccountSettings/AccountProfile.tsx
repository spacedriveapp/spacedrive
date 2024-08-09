import { Envelope } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';
import { User } from '~/navigation/tabs/SettingsStack';

const AccountProfile = () => {
	const [userInfo, setUserInfo] = useState<User | null>(null);
	useEffect(() => {
		async function _() {
			const user_data = await fetch('http://localhost:9420/api/user', {
				method: 'GET'
			});
			const data = await user_data.json();
			return data;
		}
		_().then((data) => {
			if (data.message !== 'unauthorised') {
				setUserInfo(data as User);
			}
		});
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	const emailName = userInfo ? userInfo.email.split('@')[0] : '';
	const capitalizedEmailName = (emailName?.charAt(0).toUpperCase() ?? '') + emailName?.slice(1);

	return (
		<ScreenContainer scrollview={false} style={tw`gap-2 px-6`}>
			<View style={tw`flex flex-col justify-between gap-5 lg:flex-row`}>
				<Card
					style={tw`relative flex w-full flex-col items-center justify-center !p-0 lg:max-w-[320px]`}
				>
					<View style={tw`p-3`}>
						<Text style={tw`mx-auto mt-3 text-lg text-white`}>
							Welcome{' '}
							<Text style={tw`font-bold text-white`}>{capitalizedEmailName}</Text>
						</Text>
						<View style={tw`mx-auto mt-4 flex w-full flex-col gap-2`}>
							<Card
								style={tw`w-full items-center justify-start gap-1 bg-app-input !px-2`}
							>
								<View style={tw`w-[20px]`}>
									<Envelope weight="fill" size={20} color='white' />
								</View>
								<Text style={tw`text-white`}>{userInfo ? userInfo.email : ''}</Text>
							</Card>
						</View>
					</View>
				</Card>
			</View>
		</ScreenContainer>
	);
};

export default AccountProfile;
