import tw from '@app/lib/tailwind';
import React from 'react';
import { Image, Text, View } from 'react-native';

import Divider from '../primitive/Divider';

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
