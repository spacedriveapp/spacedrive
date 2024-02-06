import { useNavigation } from '@react-navigation/native';
import { StackHeaderProps } from '@react-navigation/stack';
import { ArrowLeft, DotsThreeOutline, MagnifyingGlass } from 'phosphor-react-native';
import { lazy } from 'react';
import { Platform, Pressable, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

import { Icon } from '../icons/Icon';

//Not all pages use these components - so we lazy load for performance
const BrowseLibraryManager = lazy(() => import('../browse/DrawerLibraryManager'));
const Search = lazy(() => import('../inputs/Search'));

type HeaderProps = {
	title?: string; //title of the page
	showLibrary?: boolean; //show the library manager
	searchType?: 'explorer' | 'location'; //Temporary
	navBack?: boolean; //navigate back to the previous screen
	headerKind?: 'default' | 'location' | 'tag'; //kind of header
	route?: never;
	routeTitle?: never;
};

//you can pass in a routeTitle only if route is passed in
type Props =
	| HeaderProps
	| ({
			route: StackHeaderProps;
			routeTitle?: boolean;
	  } & Omit<HeaderProps, 'route' | 'routeTitle'>);

// Default header with search bar and button to open drawer
export default function Header({
	title,
	showLibrary,
	searchType,
	navBack,
	route,
	routeTitle,
	headerKind = 'default'
}: Props) {
	const navigation = useNavigation();
	const explorerStore = useExplorerStore();
	const routeParams = route?.route.params as any;
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
	const HeaderIconKind = () => {
		switch (headerKind) {
			case 'location':
				return <Icon size={32} name="Folder" />;
			case 'tag':
				return (
					<View
						style={twStyle('h-6 w-6 rounded-full', {
							backgroundColor: routeParams.color
						})}
					/>
				);
			default:
				return null;
		}
	};

	return (
		<View
			style={tw`relative h-auto w-full border-b border-app-line/50 bg-mobile-header ${
				Platform.OS === 'android' ? 'pt-5' : 'pt-10'
			}`}
		>
			<View style={tw`mx-auto mt-5 h-auto w-full justify-center px-7 pb-5`}>
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
						<View style={tw`flex-row items-center gap-2`}>
							<HeaderIconKind />
							<Text
								numberOfLines={1}
								style={tw`max-w-[190px] text-lg font-bold text-white`}
							>
								{title || (routeTitle && route?.options.title)}
							</Text>
						</View>
					</View>
					<View style={tw`flex-row items-center gap-3`}>
						<Pressable onPress={() => navigation.navigate('Search')}>
							<MagnifyingGlass
								size={20}
								weight="bold"
								color={tw.color('text-zinc-300')}
							/>
						</Pressable>
						{(headerKind === 'location' || headerKind === 'tag') && (
							<Pressable
								onPress={() => {
									getExplorerStore().toggleMenu = !explorerStore.toggleMenu;
								}}
							>
								<DotsThreeOutline
									size={24}
									color={tw.color(
										explorerStore.toggleMenu ? 'text-accent' : 'text-zinc-300'
									)}
								/>
							</Pressable>
						)}
					</View>
				</View>

				{showLibrary && <BrowseLibraryManager style="mt-4" />}
				{searchType && <SearchType />}
			</View>
		</View>
	);
}
