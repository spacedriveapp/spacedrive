import { MagnifyingGlass } from 'phosphor-react-native';
import React, { useState } from 'react';
import { ActivityIndicator, Pressable, Text, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { Button } from '../../components/base/Button';
import tw from '../../lib/tailwind';
import { RootStackScreenProps } from '../../navigation';

const SearchScreen = ({ navigation }: RootStackScreenProps<'Search'>) => {
	const { top } = useSafeAreaInsets();

	const [loading, setLoading] = useState(false);

	return (
		<View style={tw.style('flex-1', { marginTop: top + 20 })}>
			{/* Header */}
			<View style={tw`flex flex-row items-center mx-4`}>
				{/* Search Input */}
				<View style={tw`flex-1 bg-gray-550 rounded-md h-10 mr-3`}>
					<View style={tw`flex flex-row h-full items-center px-3`}>
						<View style={tw`mr-3`}>
							{loading ? (
								<ActivityIndicator size={'small'} color={'white'} />
							) : (
								<MagnifyingGlass size={20} weight="light" color={'white'} />
							)}
						</View>
						<TextInput
							placeholder={'Search'}
							clearButtonMode="never" // can't change the color??
							underlineColorAndroid="transparent"
							placeholderTextColor={tw.color('gray-300')}
							style={tw`text-white flex-1 text-sm`}
							textContentType={'none'}
							autoFocus
							autoCapitalize="none"
							autoCorrect={false}
						/>
					</View>
				</View>
				{/* Cancel Button */}
				<Pressable onPress={() => navigation.goBack()}>
					<Text style={tw`text-primary-500`}>Cancel</Text>
				</Pressable>
			</View>
			{/* Content */}
			<View style={tw`flex-1 items-center justify-center`}>
				<Button variant="primary" onPress={() => setLoading((v) => !v)}>
					<Text>Toggle loading</Text>
				</Button>
				<Text style={tw`font-bold text-white`}>Stuff</Text>
			</View>
		</View>
	);
};

export default SearchScreen;
