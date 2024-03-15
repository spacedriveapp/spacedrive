import { StackActions, useNavigation } from '@react-navigation/native';
import { StackHeaderProps } from '@react-navigation/stack';
import { ArrowLeft, DotsThreeOutline, MagnifyingGlass } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

import BrowseLibraryManager from '../browse/DrawerLibraryManager';
import { Icon } from '../icons/Icon';
import Search from '../search/Search';

interface HeaderProps {
	title?: string; //title of the page
	showLibrary?: boolean; //show the library manager
	showSearch?: boolean; //show the search button
	searchType?: 'explorer' | 'location'; //Temporary
	headerKind?: 'default' | 'location' | 'tag'; //kind of header
}

// Props for the header with route
interface HeaderPropsWithRoute extends HeaderProps {
	route: StackHeaderProps;
	routeTitle?: boolean; // Use the title from the route
}

// Props for the header with navigation
interface HeaderPropsWithNav extends HeaderProps {
	navBack: boolean; //navigate back to the previous screen
	navBackHome?: boolean; //navigate back to the home screen of the stack
}

// Optional versions of the Route and Nav props
interface OptionalRouteProps {
	route?: StackHeaderProps;
	routeTitle?: never; // Prevents using routeTitle without route
}

interface OptionalNavProps {
	navBack?: boolean;
	navBackHome?: never; // Prevents using navBackHome without navBack
}

// Union types to allow all combinations
type CombinedProps = HeaderProps &
	(HeaderPropsWithRoute | OptionalRouteProps) &
	(HeaderPropsWithNav | OptionalNavProps);

// Default header with search bar and button to open drawer
export default function Header({
	title,
	showLibrary,
	searchType,
	navBack,
	navBackHome,
	route,
	routeTitle,
	headerKind = 'default',
	showSearch = true
}: CombinedProps) {
	const navigation = useNavigation();
	const explorerStore = useExplorerStore();
	const routeParams = route?.route.params as any;
	const headerHeight = useSafeAreaInsets().top;

	return (
		<View
			style={twStyle(
				'relative h-auto w-full border-b border-mobile-cardborder bg-mobile-header',
				{
					paddingTop: headerHeight
				}
			)}
		>
			<View style={tw`mx-auto h-auto w-full justify-center px-5 pb-4`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<View style={tw`flex-row items-center gap-3`}>
						{navBack && (
							<Pressable
								onPress={() => {
									if (navBackHome) {
										//navigate to the home screen of the stack
										navigation.dispatch(StackActions.popToTop());
									} else {
										navigation.goBack();
									}
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
					<View style={tw`relative flex-row items-center gap-1.5`}>
						{showSearch && (
							<View style={tw`flex-row items-center gap-2`}>
								<Pressable
									onPress={() => {
										navigation.navigate('Search', {
											screen: 'Home'
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
