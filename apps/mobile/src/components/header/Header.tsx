import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { RouteProp, useNavigation } from '@react-navigation/native';
import { ArrowLeft, List, MagnifyingGlass } from 'phosphor-react-native';
import { Platform, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';

type Props = {
	route?: RouteProp<any, any>; // supporting title from the options object of navigation
	navBack?: boolean; // whether to show the back icon
	navBackTo?: string; // route to go back to
	search?: boolean; // whether to show the search icon
	title?: string; // in some cases - we want to override the route title
};

// Default header with search bar and button to open drawer
export default function Header({ route, navBack, title, navBackTo, search = false }: Props) {
	const navigation = useNavigation<DrawerNavigationHelpers>();
	const headerHeight = useSafeAreaInsets().top;
	const isAndroid = Platform.OS === 'android';

	return (
		<View
			style={twStyle('relative h-auto w-full border-b border-app-cardborder bg-app-header', {
				paddingTop: headerHeight + (isAndroid ? 15 : 0)
			})}
		>
			<View style={tw`mx-auto h-auto w-full justify-center px-5 pb-3`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<View style={tw`flex-row items-center gap-3`}>
						{navBack ? (
							<Pressable
								hitSlop={24}
								onPress={() => {
									if (navBackTo) return navigation.navigate(navBackTo);
									navigation.goBack();
								}}
							>
								<ArrowLeft size={24} color={tw.color('ink')} />
							</Pressable>
						) : (
							<Pressable onPress={() => navigation.openDrawer()}>
								<List size={24} color={tw.color('ink')} />
							</Pressable>
						)}
						<Text style={tw`text-xl font-bold text-ink`}>{title || route?.name}</Text>
					</View>
					{search && (
						<Pressable
							hitSlop={24}
							onPress={() => {
								navigation.navigate('SearchStack', {
									screen: 'Search'
								});
							}}
						>
							<MagnifyingGlass
								size={20}
								weight="bold"
								color={tw.color('text-zinc-300')}
							/>
						</Pressable>
					)}
				</View>
			</View>
		</View>
	);
}
