import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import { useSharedValue } from 'react-native-reanimated';
import Header from '~/components/header/Header';
import BrowseScreen from '~/screens/browse/Browse';
import LibraryScreen from '~/screens/browse/Library';
import LocationScreen from '~/screens/browse/Location';
import LocationsScreen from '~/screens/browse/Locations';
import TagScreen from '~/screens/browse/Tag';
import TagsScreen from '~/screens/browse/Tags';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	const scrollY = useSharedValue(0);
	return (
		<Stack.Navigator initialRouteName="Browse">
			<Stack.Screen
				name="Browse"
				options={{
					header: () => <Header scrollY={scrollY} showSearch showDrawer title="Browse" />
				}}
			>
				{() => <BrowseScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Location"
				options={{
					header: (route) => (
						<Header
							route={route}
							scrollY={scrollY}
							showSearch
							headerKind="location"
							routeTitle
							navBack
						/>
					)
				}}
			>
				{(props) => <LocationScreen {...props} scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Tags"
				options={{
					header: () => <Header scrollY={scrollY} navBack title="Tags" />
				}}
			>
				{() => <TagsScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Locations"
				options={{
					header: () => (
						<Header scrollY={scrollY} navBack searchType="location" title="Locations" />
					)
				}}
			>
				{() => <LocationsScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Tag"
				options={{
					header: (route) => <Header navBack routeTitle route={route} headerKind="tag" />
				}}
			>
				{(props) => <TagScreen {...props} scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Library"
				component={LibraryScreen}
				options={{
					header: () => <Header navBack title="Library" />
				}}
			/>
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number; path?: string };
	Locations: undefined;
	Tag: { id: number; color: string };
	Tags: undefined;
	Library: undefined;
};

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
