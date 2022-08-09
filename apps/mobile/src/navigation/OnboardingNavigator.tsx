import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import OnboardingScreen from '../screens/onboarding/Onboarding';

const OnboardingStack = createNativeStackNavigator<OnboardingStackParamList>();

export default function OnboardingNavigator() {
	return (
		<OnboardingStack.Navigator screenOptions={{ headerShown: false }}>
			<OnboardingStack.Screen name="Onboarding" component={OnboardingScreen} />
		</OnboardingStack.Navigator>
	);
}

export type OnboardingStackParamList = {
	Onboarding: undefined;
};

export type OnboardingStackScreenProps<Screen extends keyof OnboardingStackParamList> =
	NativeStackScreenProps<OnboardingStackParamList, Screen>;
