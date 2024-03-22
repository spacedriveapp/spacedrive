import { useNavigation } from '@react-navigation/native';
import { NativeStackHeaderProps } from '@react-navigation/native-stack';
import { ArrowLeft, DotsThreeOutline, MagnifyingGlass } from 'phosphor-react-native';
import { Platform, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

import BrowseLibraryManager from '../browse/DrawerLibraryManager';
import { Icon } from '../icons/Icon';
import Search from '../search/Search';

type HeaderProps = {
	title?: string; //title of the page
	showLibrary?: boolean; //show the library manager
	showSearch?: boolean; //show the search button
	searchType?: 'explorer' | 'location' | 'categories'; //Temporary
	navBack?: boolean; //navigate back to the previous screen
	navBackHome?: boolean; //navigate back to the home screen of the stack
	headerKind?: 'default' | 'location' | 'tag'; //kind of header
	route?: never;
	routeTitle?: never;
};

//you can pass in a routeTitle only if route is passed in
type Props =
	| HeaderProps
	| ({
			route: NativeStackHeaderProps;
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
	navBackHome = false,
	headerKind = 'default',
	showSearch = true
}: Props) {
	const navigation = useNavigation();
	const explorerStore = useExplorerStore();
	const routeParams = route?.route.params as any;
	const headerHeight = useSafeAreaInsets().top;
	const isAndroid = Platform.OS === 'android';

	return (
		<View
			style={twStyle('relative h-auto w-full border-b border-app-cardborder bg-app-header', {
				paddingTop: headerHeight + (isAndroid ? 15 : 0)
			})}
		>
			<View style={tw`mx-auto h-auto w-full justify-center px-5 pb-4`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<View style={tw`flex-row items-center gap-3`}>
						{navBack && (
							<Pressable
								hitSlop={24}
								onPress={() => {
									navigation.goBack();
								}}
							>
								<ArrowLeft size={23} color={tw.color('ink')} />
							</Pressable>
						)}
						<View style={tw`flex-row items-center gap-2`}>
							<HeaderIconKind headerKind={headerKind} routeParams={routeParams} />
							<Text
								numberOfLines={1}
								style={tw`max-w-[200px] text-xl font-bold text-white`}
							>
								{title || (routeTitle && route?.options.title)}
							</Text>
						</View>
					</View>
					<View style={tw`relative flex-row items-center gap-3`}>
						{showSearch && (
							<View style={tw`flex-row items-center gap-2`}>
								<Pressable
									hitSlop={24}
									onPress={() => {
										navigation.navigate('ExplorerSearch', {
											screen: 'Search'
										});
									}}
								>
									<MagnifyingGlass
										size={24}
										weight="bold"
										color={tw.color('text-zinc-300')}
									/>
								</Pressable>
							</View>
						)}
						{(headerKind === 'location' || headerKind === 'tag') && (
							<Pressable
								hitSlop={24}
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
				{searchType && <HeaderSearchType searchType={searchType} />}
			</View>
		</View>
	);
}

interface HeaderSearchTypeProps {
	searchType: HeaderProps['searchType'];
}

const HeaderSearchType = ({ searchType }: HeaderSearchTypeProps) => {
	switch (searchType) {
		case 'explorer':
			return 'Explorer'; //TODO
		case 'location':
			return <Search placeholder="Location name..." />;
		case 'categories':
			return <Search placeholder="Category name..." />;
		default:
			return null;
	}
};

interface HeaderIconKindProps {
	headerKind: HeaderProps['headerKind'];
	routeParams?: any;
}

const HeaderIconKind = ({ headerKind, routeParams }: HeaderIconKindProps) => {
	switch (headerKind) {
		case 'location':
			return <Icon size={30} name="Folder" />;
		case 'tag':
			return (
				<View
					style={twStyle('h-[30px] w-[30px] rounded-full', {
						backgroundColor: routeParams.color
					})}
				/>
			);
		default:
			return null;
	}
};
