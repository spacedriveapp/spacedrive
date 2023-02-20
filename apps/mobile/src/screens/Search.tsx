import { MagnifyingGlass } from 'phosphor-react-native';
import { useState } from 'react';
import { ActivityIndicator, Pressable, Text, TextInput, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { RootStackScreenProps } from '~/navigation';

const SearchScreen = ({ navigation }: RootStackScreenProps<'Search'>) => {
	const { top } = useSafeAreaInsets();

	const [loading, setLoading] = useState(false);

	// TODO: Animations!

	return (
		<View style={twStyle('flex-1', { marginTop: top + 10 })}>
			{/* Header */}
			<View style={tw`mx-4 flex flex-row items-center`}>
				{/* Search Input */}
				<View style={tw`border-app-line bg-app-overlay mr-3 h-10 flex-1 rounded border`}>
					<View style={tw`flex h-full flex-row items-center px-3`}>
						<View style={tw`mr-3`}>
							{loading ? (
								<ActivityIndicator size={'small'} color={'white'} />
							) : (
								<MagnifyingGlass size={20} weight="light" color={tw.color('ink-faint')} />
							)}
						</View>
						<TextInput
							placeholder={'Search'}
							clearButtonMode="never" // can't change the color??
							underlineColorAndroid="transparent"
							placeholderTextColor={tw.color('ink-dull')}
							style={tw`text-ink flex-1 text-sm font-medium`}
							textContentType={'none'}
							autoFocus
							autoCapitalize="none"
							autoCorrect={false}
						/>
					</View>
				</View>
				{/* Cancel Button */}
				<Pressable onPress={() => navigation.goBack()}>
					<Text style={tw`text-accent`}>Cancel</Text>
				</Pressable>
			</View>
			{/* Content */}
			<View style={tw`mt-8 flex-1 items-center`}>
				<Button variant="accent" onPress={() => setLoading((v) => !v)}>
					<Text>Toggle loading</Text>
				</Button>
			</View>
		</View>
	);
};

export default SearchScreen;
