import { Alert } from 'react-native';

export function FeatureUnavailableAlert() {
	return Alert.alert(
		'Coming soon',
		'This feature is not available right now. Please check back later.',
		[
			{
				text: 'Close'
			}
		],
		{
			userInterfaceStyle: 'dark'
		}
	);
}
