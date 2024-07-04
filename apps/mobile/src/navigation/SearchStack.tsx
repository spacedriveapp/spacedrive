import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import DynamicHeader from '~/components/header/DynamicHeader';
import Header from '~/components/header/Header';
import LocationScreen from '~/screens/browse/Location';
import FiltersScreen from '~/screens/search/Filters';
import SearchScreen from '~/screens/search/Search';

const Stack = createNativeStackNavigator<SearchStackParamList>();

export default function SearchStack() {
	return (
		<Stack.Navigator initialRouteName="Search">
			<Stack.Screen
				name="Search"
				component={SearchScreen}
				options={{
					headerShown: false
				}}
			/>
			<Stack.Screen
				name="Filters"
				component={FiltersScreen}
				options={{
					header: () => {
						return <Header navBack title="Search filters" />;
					}
				}}
			/>
			{/** This screen is already in BrowseStack - but added here as it offers the UX needed */}
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
		</Stack.Navigator>
	);
}

export type SearchStackParamList = {
	Search: undefined;
	Filters: undefined;
	Location: { id: number; path: string };
};

export type SearchStackScreenProps<Screen extends keyof SearchStackParamList> =
	NativeStackScreenProps<SearchStackParamList, Screen>;
