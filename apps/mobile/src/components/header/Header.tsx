import { useNavigation } from '@react-navigation/native';
import { MagnifyingGlass } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';

// Default header with search bar and button to open drawer
export default function Header() {
	const navigation = useNavigation();

	const { top } = useSafeAreaInsets();

	return (
		<View
			style={twStyle('mx-4 rounded border border-app-line bg-app-overlay', {
				marginTop: top + 10
			})}
		>
			<View style={tw`flex h-10 flex-row items-center px-3`}>
				<MagnifyingGlass
					size={20}
					weight="light"
					color={tw.color('ink-faint')}
					style={tw`mr-3`}
				/>
				<Pressable
					style={tw`h-full flex-1 justify-center`}
					onPress={() => navigation.navigate('Search')}
				>
					<Text style={tw`text-sm font-medium text-ink-dull`}>Search</Text>
				</Pressable>
			</View>
		</View>
	);
}
