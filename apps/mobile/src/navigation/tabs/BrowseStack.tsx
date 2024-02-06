import { CompositeScreenProps } from '@react-navigation/native';
import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import Header from '~/components/header/Header';
import { tw } from '~/lib/tailwind';
import BrowseScreen from '~/screens/browse';
import LocationScreen from '~/screens/Location';
import { Locations } from '~/screens/Locations';
import TagScreen from '~/screens/Tag';

import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator
			initialRouteName="Browse"
			screenOptions={{
				headerStyle: { backgroundColor: tw.color('app-box') },
				headerTintColor: tw.color('ink'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
			}}
		>
			<Stack.Screen
				name="Browse"
				component={BrowseScreen}
				options={{ header: () => <Header showLibrary title="Browse" /> }}
			/>
			<Stack.Screen
				name="Location"
				component={LocationScreen}
				options={{
					header: (route) => (
						<Header route={route} headerKind="location" routeTitle navBack />
					)
				}}
			/>
			<Stack.Screen
				name="Locations"
				component={Locations}
				options={{
					header: () => <Header navBack searchType="location" title="Locations" />
				}}
			/>
			<Stack.Screen
				name="Tag"
				component={TagScreen}
				options={{
					header: (route) => <Header routeTitle route={route} headerKind="tag" navBack />
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
};

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		StackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
