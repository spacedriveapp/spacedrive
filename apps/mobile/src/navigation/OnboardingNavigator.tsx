import { createNativeStackNavigator } from '@react-navigation/native-stack';

import OnboardingScreen from '../screens/Onboarding/Onboarding';
import { OnboardingStackParamList } from '../types/navigation';

const OnboardingStack = createNativeStackNavigator<OnboardingStackParamList>();

export default function OnboardingNavigator() {
	return (
		<OnboardingStack.Navigator screenOptions={{ headerShown: false }}>
			<OnboardingStack.Screen name="Onboarding" component={OnboardingScreen} />
		</OnboardingStack.Navigator>
	);
}
