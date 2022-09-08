import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SharedScreenProps } from '~/navigation/SharedScreens';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id } = route.params;
	return (
		<View style={tw`items-center justify-center flex-1`}>
			<Text style={tw`text-xl font-bold text-white`}>Location {id}</Text>
		</View>
	);
}
