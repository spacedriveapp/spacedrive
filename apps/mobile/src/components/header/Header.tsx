import { useNavigation } from '@react-navigation/native';
import { ArrowLeft, MagnifyingGlass } from 'phosphor-react-native';
import { lazy } from 'react';
import { Pressable, Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

//Not all pages use these components - so we lazy load for performance
const BrowseLibraryManager = lazy(() => import('../browse/DrawerLibraryManager'));
const Search = lazy(() => import('../inputs/Search'));

interface Props {
	title?: string; //title of the page
	showLibrary?: boolean; //show the library manager
	searchType?: 'explorer' | 'location'; //Temporary
	navBack?: boolean; //navigate back to the previous screen
}

// Default header with search bar and button to open drawer
export default function Header({ title, showLibrary, searchType, navBack }: Props) {
	const navigation = useNavigation();

	const SearchType = () => {
		switch (searchType) {
			case 'explorer':
				return 'Explorer'; //TODO
			case 'location':
				return <Search placeholder="Location name..." />;
			default:
				return null;
		}
	};

	return (
		<View style={tw`relative h-fit w-full border-b border-app-line/50 bg-mobile-header pt-10`}>
			<View style={tw`mx-auto mt-5 h-fit w-full justify-center px-7 pb-5`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<View style={tw`flex-row items-center gap-5`}>
						{navBack && (
							<Pressable
								onPress={() => {
									navigation.goBack();
								}}
							>
								<ArrowLeft size={23} color={tw.color('ink')} />
							</Pressable>
						)}
						<Text style={tw`text-[24px] font-bold text-white`}>{title}</Text>
					</View>
					<Pressable onPress={() => navigation.navigate('Search')}>
						<MagnifyingGlass
							size={20}
							weight="bold"
							color={tw.color('text-zinc-300')}
						/>
					</Pressable>
				</View>
				{showLibrary && <BrowseLibraryManager style="mt-4" />}
				{searchType && <SearchType />}
			</View>
		</View>
	);
}
