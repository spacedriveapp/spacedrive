import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import { OnboardingContext, useContextValue } from '~/components/context/OnboardingContext';
import CreatingLibraryScreen from '~/screens/onboarding/CreatingLibrary';
import GetStartedScreen from '~/screens/onboarding/GetStarted';
import NewLibraryScreen from '~/screens/onboarding/NewLibrary';
import PrivacyScreen from '~/screens/onboarding/Privacy';

const OnboardingStack = createNativeStackNavigator<OnboardingStackParamList>();

export default function OnboardingNavigator() {
	return (
		<OnboardingContext.Provider value={useContextValue()}>
			<OnboardingStack.Navigator
				id="onboarding"
				initialRouteName="GetStarted"
				screenOptions={{ headerShown: false }}
			>
				<OnboardingStack.Screen name="GetStarted" component={GetStartedScreen} />
				<OnboardingStack.Screen name="NewLibrary" component={NewLibraryScreen} />
				<OnboardingStack.Screen name="Privacy" component={PrivacyScreen} />
				<OnboardingStack.Screen
					name="CreatingLibrary"
					component={CreatingLibraryScreen}
					options={{
						// Disable swipe back gesture
						gestureEnabled: false
					}}
				/>
			</OnboardingStack.Navigator>
		</OnboardingContext.Provider>
	);
}

export type OnboardingStackParamList = {
	GetStarted: undefined;
	NewLibrary: undefined;
	Privacy: undefined;
	CreatingLibrary: undefined;
};

export type OnboardingStackScreenProps<Screen extends keyof OnboardingStackParamList> =
	NativeStackScreenProps<OnboardingStackParamList, Screen>;
