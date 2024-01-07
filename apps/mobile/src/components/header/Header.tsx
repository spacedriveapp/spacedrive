import { useNavigation } from '@react-navigation/native';
import { MagnifyingGlass } from 'phosphor-react-native';
import { lazy } from 'react';
import { Pressable, Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

//Not all pages have a library manager - so lazy load it for performance
const BrowseLibraryManager = lazy(() => import('../browse/DrawerLibraryManager'));

interface Props {
	title?: string;
	showLibrary?: boolean;
}

// Default header with search bar and button to open drawer
export default function Header({ title, showLibrary }: Props) {
	const navigation = useNavigation();
	return (
		<View style={tw`relative h-fit w-full border-b border-app-line/50 bg-mobile-header pt-10`}>
			<View style={tw`mx-auto mt-5 h-fit w-full justify-center px-7 pb-5`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<Text style={tw`text-[24px] font-bold text-white`}>{title}</Text>
					<Pressable onPress={() => navigation.navigate('Search')}>
						<MagnifyingGlass
							size={20}
							weight="bold"
							color={tw.color('text-zinc-300')}
						/>
					</Pressable>
				</View>
				{showLibrary && <BrowseLibraryManager style="mt-4" />}
			</View>
		</View>
	);
}
