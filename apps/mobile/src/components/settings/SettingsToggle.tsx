import { useState } from 'react';
import { Switch, Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

interface Props {
	title: string;
	description?: string;
	onEnabledChange?: (enabled: boolean) => void;
}

const SettingsToggle = ({ title, description, onEnabledChange }: Props) => {
	const [isEnabled, setIsEnabled] = useState(false);
	return (
		<View style={tw`flex-row items-center justify-between`}>
			<View style={tw`w-[75%]`}>
				<Text style={tw`text-sm font-medium text-ink`}>{title}</Text>
				{description && <Text style={tw`mt-1 text-xs text-ink-dull`}>{description}</Text>}
			</View>
			<Switch
				trackColor={{
					true: tw.color('bg-accent'),
					false: tw.color('bg-app-input')
				}}
				value={isEnabled}
				onValueChange={(enabled) => {
					setIsEnabled(!isEnabled);
					onEnabledChange?.(!isEnabled);
				}}
			/>
		</View>
	);
};

export default SettingsToggle;
