import React from 'react';
import { Image, Text, View } from 'react-native';

import tw from '../../lib/tailwind';
import Divider from '../base/Divider';

const DrawerLogo = () => {
	return (
		<>
			<View style={tw`flex flex-row items-center`}>
				<Image source={require('@sd/assets/images/logo.png')} style={tw`w-9 h-9`} />
				<Text style={tw`text-base font-bold text-white ml-2`}>Spacedrive</Text>
			</View>
			<Divider style={tw`mt-4`} />
		</>
	);
};

export default DrawerLogo;
