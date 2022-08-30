import tw from '@app/lib/tailwind';
import { SharedScreenProps } from '@app/navigation/SharedScreens';
import React from 'react';
import { Text, View } from 'react-native';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id } = route.params;
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Location {id}</Text>
		</View>
	);
}
