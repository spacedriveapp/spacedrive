import tw from '@app/lib/tailwind';
import { SharedScreenProps } from '@app/navigation/SharedScreens';
import React from 'react';
import { Text, View } from 'react-native';

export default function TagScreen({ navigation, route }: SharedScreenProps<'Tag'>) {
	const { id } = route.params;
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Tag {id}</Text>
		</View>
	);
}
