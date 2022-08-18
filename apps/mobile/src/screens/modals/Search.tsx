import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { ChevronLeftIcon } from 'react-native-heroicons/outline';
import { SafeAreaView } from 'react-native-safe-area-context';

import tw from '../../lib/tailwind';
import { RootStackScreenProps } from '../../navigation';

const SearchScreen = ({ navigation }: RootStackScreenProps<'Search'>) => {
	return (
		<SafeAreaView edges={['top']} style={tw`flex-1 pt-4`}>
			{/* Header */}
			<View style={tw`flex flex-row items-center px-4`}>
				{/* Back Button */}
				<Pressable onPress={() => navigation.goBack()}>
					<ChevronLeftIcon color={tw.color('primary-500')} width={25} height={25} />
				</Pressable>
				{/* Search Input */}
				<View>
					<Text style={tw`text-white`}>Search</Text>
				</View>
			</View>
			<View style={tw`flex-1 items-center justify-center`}>
				<Text style={tw`font-bold text-white`}>Stuff</Text>
			</View>
		</SafeAreaView>
	);
};

export default SearchScreen;
