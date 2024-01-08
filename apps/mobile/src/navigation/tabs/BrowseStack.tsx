import { CompositeScreenProps } from '@react-navigation/native';
import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import { ArrowLeft } from 'phosphor-react-native';
import Header from '~/components/header/Header';
import { tw } from '~/lib/tailwind';
import BrowseScreen from '~/screens/browse';
import LocationScreen from '~/screens/Location';
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
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<Stack.Screen
				name="Tag"
				component={TagScreen}
				options={{
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number; path?: string };
	Tag: { id: number };
};

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		StackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
