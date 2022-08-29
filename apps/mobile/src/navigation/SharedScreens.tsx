import { ParamListBase, StackNavigationState, TypedNavigator } from '@react-navigation/native';
import {
	StackNavigationEventMap,
	StackNavigationOptions,
	StackScreenProps
} from '@react-navigation/stack';

import LocationScreen from '../screens/Location';
import TagScreen from '../screens/Tag';

export function SharedScreens(
	Stack: TypedNavigator<
		SharedScreensParamList,
		StackNavigationState<ParamListBase>,
		StackNavigationOptions,
		StackNavigationEventMap,
		any
	>
) {
	return (
		<>
			<Stack.Screen name="Location" component={LocationScreen} />
			<Stack.Screen name="Tag" component={TagScreen} />
		</>
	);
}

export type SharedScreensParamList = {
	Location: { id: number };
	Tag: { id: number };
};

export type SharedScreenProps<Screen extends keyof SharedScreensParamList> = StackScreenProps<
	SharedScreensParamList,
	Screen
>;
