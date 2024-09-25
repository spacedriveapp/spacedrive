import { useNavigation } from '@react-navigation/native';
import { useEffect } from 'react';

/**
 * This hook enables the drawer swipe gesture when the screen is focused and disables it when the screen is blurred.
 */

export function useEnableDrawer(): void {
	const navigation = useNavigation();
	useEffect(() => {
		const tabNavigator = navigation.getParent(); // This is the TabNavigator
		const drawerNavigator = tabNavigator?.getParent(); // This is the DrawerNavigator

		const unsubscribeFocus = navigation.addListener('focus', () => {
			drawerNavigator?.setOptions({ swipeEnabled: true });
		});

		const unsubscribeBlur = navigation.addListener('blur', () => {
			drawerNavigator?.setOptions({ swipeEnabled: false });
		});

		return () => {
			unsubscribeFocus();
			unsubscribeBlur();
		};
	}, [navigation]);
}
