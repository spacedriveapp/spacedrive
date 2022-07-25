import { DrawerScreenProps } from '@react-navigation/drawer';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { NativeStackScreenProps } from '@react-navigation/native-stack';

// This declaration is used by useNavigation, Link, ref etc.
declare global {
	// eslint-disable-next-line @typescript-eslint/no-namespace
	namespace ReactNavigation {
		// eslint-disable-next-line @typescript-eslint/no-empty-interface
		interface RootParamList extends RootStackParamList {}
	}
}

// Root Stack

export type RootStackParamList = {
	Root: NavigatorScreenParams<HomeDrawerParamList> | undefined;
	Modal: undefined;
	NotFound: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = NativeStackScreenProps<
	RootStackParamList,
	Screen
>;

// Main Navigation (Drawer)

export type HomeDrawerParamList = {
	Overview: undefined;
	Content: undefined;
	Photos: undefined;
	Location: undefined;
	Tag: undefined;
	Settings: undefined;
};

export type HomeDrawerScreenProps<Screen extends keyof HomeDrawerParamList> = CompositeScreenProps<
	DrawerScreenProps<HomeDrawerParamList, Screen>,
	NativeStackScreenProps<RootStackParamList>
>;

// Onboarding Stack
// Seperated stack for onboarding screens

export type OnboardingStackParamList = {
	Onboarding: undefined;
};

export type OnboardingStackScreenProps<Screen extends keyof OnboardingStackParamList> =
	NativeStackScreenProps<OnboardingStackParamList, Screen>;
