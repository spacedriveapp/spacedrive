import { useDrawerStatus } from '@react-navigation/drawer';
import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { List } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import tw from '~/lib/tailwind';

// Default header with search bar and button to open drawer
export default function Header() {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const { top } = useSafeAreaInsets();

	const isDrawerOpen = useDrawerStatus() === 'open';

	return (
		<View
			style={tw.style('mx-4 bg-app-overlay border border-app-line rounded', {
				marginTop: top + 10
			})}
		>
			<View style={tw`flex flex-row items-center h-10`}>
				<Pressable style={tw`px-3 h-full justify-center`} onPress={() => navigation.openDrawer()}>
					<MotiView
						animate={{ rotate: isDrawerOpen ? '90deg' : '0deg' }}
						transition={{ type: 'timing' }}
					>
						<List size={20} color={tw.color('ink-faint')} weight="fill" />
					</MotiView>
				</Pressable>
				<Pressable
					style={tw`flex-1 h-full justify-center`}
					onPress={() => navigation.navigate('Search')}
				>
					<Text style={tw`text-ink-dull font-medium text-sm`}>Search</Text>
				</Pressable>
			</View>
		</View>
	);
}
