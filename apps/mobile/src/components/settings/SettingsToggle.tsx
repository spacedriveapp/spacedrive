import { useState } from 'react';
import { Control, Controller } from 'react-hook-form';
import { Switch, Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';

type Props =
	| {
			title: string; // Title of the setting
			description?: string; // This is to display a description below the title
			onEnabledChange?: (enabled: boolean) => void; // This is to receive the value of the toggle when it changes
			control: Control<any>; //Zod form control
			name: string; //Name of the field for zod form controller
	  }
	| {
			title: string;
			description?: string;
			onEnabledChange?: (enabled: boolean) => void;
			control?: never;
			name?: never;
	  };

const SettingsToggle = ({ title, description, onEnabledChange, control, name }: Props) => {
	const [isEnabled, setIsEnabled] = useState(false);

	return (
		<View style={tw`flex-row items-center justify-between`}>
			<View style={tw`w-3/4`}>
				<Text style={tw`text-sm font-medium text-ink`}>{title}</Text>
				{description && <Text style={tw`mt-1 text-xs text-ink-dull`}>{description}</Text>}
			</View>
			{control && name ? (
				<Controller
					name={name}
					control={control}
					render={({ field: { onChange, value } }) => (
						<Switch
							trackColor={{
								true: tw.color('accent')
							}}
							value={value ?? isEnabled}
							onValueChange={(val) => {
								setIsEnabled(val);
								onChange(val);
								onEnabledChange?.(val);
							}}
						/>
					)}
				/>
			) : (
				<Switch
					trackColor={{
						true: tw.color('accent')
					}}
					value={isEnabled}
					onValueChange={() => {
						setIsEnabled((prev) => !prev);
						onEnabledChange?.(!isEnabled);
					}}
				/>
			)}
		</View>
	);
};

export default SettingsToggle;
