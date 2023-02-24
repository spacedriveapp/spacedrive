import { StackScreenProps, createStackNavigator } from '@react-navigation/stack';
import CreatingLibraryScreen from '~/screens/onboarding/CreatingLibrary';
import GetStartedScreen from '~/screens/onboarding/GetStarted';
import MasterPasswordScreen from '~/screens/onboarding/MasterPassword';
import NewLibraryScreen from '~/screens/onboarding/NewLibrary';
import PrivacyScreen from '~/screens/onboarding/Privacy';

const OnboardingStack = createStackNavigator<OnboardingStackParamList>();

export default function OnboardingNavigator() {
	return (
		<OnboardingStack.Navigator initialRouteName="GetStarted" screenOptions={{ headerShown: false }}>
			<OnboardingStack.Screen name="GetStarted" component={GetStartedScreen} />
			<OnboardingStack.Screen name="NewLibrary" component={NewLibraryScreen} />
			<OnboardingStack.Screen name="MasterPassword" component={MasterPasswordScreen} />
			<OnboardingStack.Screen name="Privacy" component={PrivacyScreen} />
			<OnboardingStack.Screen name="CreatingLibrary" component={CreatingLibraryScreen} />
		</OnboardingStack.Navigator>
	);
}

export type OnboardingStackParamList = {
	GetStarted: undefined;
	NewLibrary: undefined;
	MasterPassword: undefined;
	Privacy: undefined;
	CreatingLibrary: undefined;
};

export type OnboardingStackScreenProps<Screen extends keyof OnboardingStackParamList> =
	StackScreenProps<OnboardingStackParamList, Screen>;
