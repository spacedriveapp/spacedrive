import LocationScreen from '@app/screens/Location';
import TagScreen from '@app/screens/Tag';
import { ParamListBase, StackNavigationState, TypedNavigator } from '@react-navigation/native';
import {
	StackNavigationEventMap,
	StackNavigationOptions,
	StackScreenProps
} from '@react-navigation/stack';

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
