import { CompositeScreenProps } from '@react-navigation/native';
import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import BrowseScreen from '../../screens/Browse';
import LocationScreen from '../../screens/Location';
import TagScreen from '../../screens/Tag';
import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator initialRouteName="Browse">
			<Stack.Screen name="Browse" component={BrowseScreen} />
			<Stack.Screen name="Location" component={LocationScreen} />
			<Stack.Screen name="Tag" component={TagScreen} />
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number };
	Tag: { id: number };
};

export type BrowseScreenProps<Screen extends keyof BrowseStackParamList> = CompositeScreenProps<
	NativeStackScreenProps<BrowseStackParamList, Screen>,
	TabScreenProps<'BrowseStack'>
>;
