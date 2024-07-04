import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import DynamicHeader from '~/components/header/DynamicHeader';
import Header from '~/components/header/Header';
import SearchHeader from '~/components/header/SearchHeader';
import BrowseScreen from '~/screens/browse/Browse';
import LibraryScreen from '~/screens/browse/Library';
import LocationScreen from '~/screens/browse/Location';
import LocationsScreen from '~/screens/browse/Locations';
import TagScreen from '~/screens/browse/Tag';
import TagsScreen from '~/screens/browse/Tags';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator initialRouteName="Browse">
			<Stack.Screen
				name="Browse"
				component={BrowseScreen}
				options={({ route }) => ({
					header: () => <Header search route={route} />
				})}
			/>
			<Stack.Screen
				name="Location"
				component={LocationScreen}
				options={({ route: optionsRoute }) => ({
					header: (route) => (
						<DynamicHeader
							optionsRoute={optionsRoute}
							headerRoute={route}
							kind="locations"
						/>
					)
				})}
			/>
			<Stack.Screen
				name="Tags"
				component={TagsScreen}
				options={({ route }) => ({
					header: () => <SearchHeader kind="tags" route={route} />
				})}
			/>
			<Stack.Screen
				name="Locations"
				component={LocationsScreen}
				options={({ route }) => ({
					header: () => <SearchHeader kind="locations" route={route} />
				})}
			/>
			<Stack.Screen
				name="Tag"
				component={TagScreen}
				options={({ route: optionsRoute }) => ({
					header: (route) => (
						<DynamicHeader
							optionsRoute={optionsRoute}
							headerRoute={route}
							kind="tags"
						/>
					)
				})}
			/>
			<Stack.Screen
				name="Library"
				component={LibraryScreen}
				options={({ route }) => ({
					header: () => <Header navBack route={route} />
				})}
			/>
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number; path?: string; name?: string };
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
