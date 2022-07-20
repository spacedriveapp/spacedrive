import { Image, Text, View } from 'react-native';

import tw from '../lib/tailwind';
import { RootTabScreenProps } from '../types';

export default function TabOneScreen({ navigation }: RootTabScreenProps<'TabOne'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			{/* Doing this import to make sure importing from workspace works... */}
			<Image source={require('@sd/interface/src/assets/images/spacedrive_logo.png')} />
			<Text style={tw`text-primary-500 font-bold text-3xl`}>Spacedrive</Text>
		</View>
	);
}
