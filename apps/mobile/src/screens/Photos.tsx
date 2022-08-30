import tw from '@app/lib/tailwind';
import { PhotosStackScreenProps } from '@app/navigation/tabs/PhotosStack';
import React from 'react';
import { Text, View } from 'react-native';

export default function PhotosScreen({ navigation }: PhotosStackScreenProps<'Photos'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Photos</Text>
		</View>
	);
}
