import { Text, View } from 'react-native';

type SettingsHeaderProps = {
	title: string;
	description?: string;
};

export function SettingsHeader(props: SettingsHeaderProps) {
	return (
		<View>
			<Text>SettingsHeader</Text>
		</View>
	);
}
