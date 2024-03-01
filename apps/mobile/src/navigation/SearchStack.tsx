import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import React from 'react';
import Header from '~/components/header/Header';
import SearchScreen from '~/screens/search';
import FiltersScreen from '~/screens/search/filters';

const Stack = createStackNavigator<SearchStackParamList>();

export default function SearchStack() {
	return (
		<Stack.Navigator initialRouteName="SearchHome">
			<Stack.Screen
				name="SearchHome"
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
						return <Header navBack showSearch={false} title="Search filters" />;
					}
				}}
			/>
		</Stack.Navigator>
	);
}

export type SearchStackParamList = {
	SearchHome: undefined;
	Filters: undefined;
};

export type SearchStackScreenProps<Screen extends keyof SearchStackParamList> = StackScreenProps<
	SearchStackParamList,
	Screen
>;
