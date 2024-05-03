import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { RouteProp, useNavigation } from '@react-navigation/native';
import { ArrowLeft } from 'phosphor-react-native';
import { Platform, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';

import Search from '../search/Search';

const searchPlaceholder = {
	locations: 'Search location name...',
	tags: 'Search tag name...',
	categories: 'Search category name...'
};

type Props = {
	route?: RouteProp<any, any>; // supporting title from the options object of navigation
	kind: keyof typeof searchPlaceholder; // the kind of search we are doing
	title?: string; // in some cases - we want to override the route title
};

export default function SearchHeader({ route, kind, title }: Props) {
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
						<Pressable hitSlop={24} onPress={() => navigation.goBack()}>
							<ArrowLeft size={24} color={tw.color('ink')} />
						</Pressable>
						<Text style={tw`text-xl font-bold text-ink`}>{title || route?.name}</Text>
					</View>
				</View>
				<Search placeholder={searchPlaceholder[kind]} />
			</View>
		</View>
	);
}
