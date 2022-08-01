import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CirclesFour, Planet } from 'phosphor-react-native';
import { PhotographIcon } from 'react-native-heroicons/outline';

import tw from '../lib/tailwind';
import OverviewScreen from '../screens/Overview';
import PhotosScreen from '../screens/Photos';
import SpacesScreen from '../screens/Spaces';
import { BottomNavParamList } from '../types/navigation';

const BottomTab = createBottomTabNavigator<BottomNavParamList>();

export default function BottomTabNavigator() {
	return (
		<BottomTab.Navigator
			initialRouteName="Overview"
			screenOptions={{
				headerShown: false,
				tabBarActiveTintColor: tw.color('bg-primary'),
				tabBarInactiveTintColor: 'white',
				tabBarStyle: {
					backgroundColor: '#08090D',
					borderTopColor: 'transparent'
				}
			}}
		>
			<BottomTab.Screen
				name="Overview"
				component={OverviewScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<Planet size={22} weight="bold" color={focused ? tw.color('bg-primary') : 'white'} />
					)
				}}
			/>
			<BottomTab.Screen
				name="Spaces"
				component={SpacesScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<CirclesFour
							size={22}
							weight="bold"
							color={focused ? tw.color('bg-primary') : 'white'}
						/>
					)
				}}
			/>
			<BottomTab.Screen
				name="Photos"
				component={PhotosScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<PhotographIcon size={22} color={focused ? tw.color('bg-primary') : 'white'} />
					)
				}}
			/>
		</BottomTab.Navigator>
	);
}
