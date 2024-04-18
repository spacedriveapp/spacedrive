import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { NativeStackHeaderProps } from '@react-navigation/native-stack';
import { ArrowLeft, DotsThreeOutline, List, MagnifyingGlass } from 'phosphor-react-native';
import React from 'react';
import { Platform, Pressable, View } from 'react-native';
import Animated, {
	Extrapolation,
	interpolate,
	SharedValue,
	useAnimatedStyle
} from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

import { Icon } from '../icons/Icon';
import { AnimatedPressable } from '../reanimated/components';
import Search from '../search/Search';

type HeaderProps = {
	title?: string; //title of the page
	showSearch?: boolean; //show the search button
	showDrawer?: boolean; //show the drawer button
	searchType?: 'location' | 'categories'; //Temporary
	navBack?: boolean; //navigate back to the previous screen
	headerKind?: 'default' | 'location' | 'tag'; //kind of header
	route?: never;
	routeTitle?: never;
	scrollY?: SharedValue<number>; //scrollY of screen
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
	searchType,
	navBack,
	route,
	routeTitle,
	headerKind = 'default',
	showDrawer = false,
	showSearch = false,
	scrollY
}: Props) {
	const navigation = useNavigation<DrawerNavigationHelpers>();
	const explorerStore = useExplorerStore();
	const routeParams = route?.route.params as any;
	const headerHeight = useSafeAreaInsets().top;
	const isAndroid = Platform.OS === 'android';

	const scrollYTitle = useAnimatedStyle(() => {
		return {
			fontSize: interpolate(scrollY?.value || 0, [0, 50], [20, 16], Extrapolation.CLAMP)
		};
	});

	const scrollYHeader = useAnimatedStyle(() => {
		return {
			height: interpolate(
				scrollY?.value || 0,
				[0, 50],
				[searchType ? 150 : 100, searchType ? 145 : 85],
				Extrapolation.CLAMP
			)
		};
	});

	const scrollYIcon = useAnimatedStyle(() => {
		return {
			transform: [
				{
					scale: interpolate(scrollY?.value || 0, [0, 50], [1, 0.9], Extrapolation.CLAMP)
				}
			]
		};
	});

	return (
		<Animated.View
			style={[
				twStyle('relative w-full border-b border-app-cardborder bg-app-header', {
					paddingTop: headerHeight + (isAndroid ? 15 : 0)
				}),
				scrollYHeader
			]}
		>
			<View style={tw`justify-center w-full h-auto px-5 pb-6 mx-auto`}>
				<View style={tw`flex-row items-center justify-between w-full`}>
					<View style={tw`flex-row items-center`}>
						{navBack && (
							<AnimatedPressable
								style={scrollYIcon}
								hitSlop={24}
								onPress={() => {
									navigation.goBack();
									if (scrollY) scrollY.value = 0;
								}}
							>
								<ArrowLeft size={23} color={tw.color('ink')} />
							</AnimatedPressable>
						)}
						<View style={tw`flex-row items-center gap-1.5`}>
							<Animated.View style={scrollYIcon}>
								<HeaderIconKind headerKind={headerKind} routeParams={routeParams} />
							</Animated.View>
							{showDrawer && (
								<AnimatedPressable
									style={scrollYIcon}
									onPress={() => navigation.openDrawer()}
								>
									<List size={24} color={tw.color('text-zinc-300')} />
								</AnimatedPressable>
							)}
							<Animated.Text
								numberOfLines={1}
								style={[twStyle('max-w-[200px] font-bold text-ink'), scrollYTitle]}
							>
								{title || (routeTitle && route?.options.title)}
							</Animated.Text>
						</View>
					</View>
					<View style={tw`relative flex-row items-center gap-3`}>
						{showSearch && (
							<View style={tw`flex-row items-center gap-2`}>
								<AnimatedPressable
									style={scrollYIcon}
									hitSlop={24}
									onPress={() => {
										navigation.navigate('SearchStack', {
											screen: 'Search'
										});
									}}
								>
									<MagnifyingGlass
										weight="bold"
										color={tw.color('text-zinc-300')}
									/>
								</AnimatedPressable>
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
				{searchType && <HeaderSearchType searchType={searchType} />}
			</View>
		</Animated.View>
	);
}

interface HeaderSearchTypeProps {
	searchType: HeaderProps['searchType'];
}

const HeaderSearchType = ({ searchType }: HeaderSearchTypeProps) => {
	switch (searchType) {
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
			return <Icon style={tw`ml-3`} size={30} name="Folder" />;
		case 'tag':
			return (
				<View
					style={twStyle('h-[30px] w-[30px] rounded-full ml-3', {
						backgroundColor: routeParams.color
					})}
				/>
			);
		default:
			return null;
	}
};
