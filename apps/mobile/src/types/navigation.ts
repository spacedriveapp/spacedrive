import { BottomTabScreenProps } from '@react-navigation/bottom-tabs';
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
	Root: NavigatorScreenParams<DrawerNavParamList> | undefined;
	Modal: undefined;
	NotFound: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = NativeStackScreenProps<
	RootStackParamList,
	Screen
>;

// Main Navigation (Bottom Tab)

export type BottomNavParamList = {
	Overview: undefined;
	Spaces: undefined;
	Photos: undefined;
};

export type BottomNavScreenProps<Screen extends keyof BottomNavParamList> = CompositeScreenProps<
	BottomTabScreenProps<BottomNavParamList, Screen>,
	HomeDrawerScreenProps<'Home'>
>;

// Drawer Navigation

export type DrawerNavParamList = {
	// Home is the nested BottomNavigation (Bottom Tab)
	Home: NavigatorScreenParams<BottomNavParamList> | undefined;
	Location: { id: number };
	Tag: { id: number };
	Settings: undefined;
};

export type HomeDrawerScreenProps<Screen extends keyof DrawerNavParamList> = CompositeScreenProps<
	DrawerScreenProps<DrawerNavParamList, Screen>,
	NativeStackScreenProps<RootStackParamList>
>;

// Onboarding Stack
// Seperated stack for onboarding screens

export type OnboardingStackParamList = {
	Onboarding: undefined;
};

export type OnboardingStackScreenProps<Screen extends keyof OnboardingStackParamList> =
	NativeStackScreenProps<OnboardingStackParamList, Screen>;
