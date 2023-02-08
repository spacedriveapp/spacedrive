import { FC } from 'react';
import { Switch as RNSwitch, SwitchProps, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

export const Switch: FC<SwitchProps> = ({ ...props }) => {
	return (
		<RNSwitch trackColor={{ false: tw.color('app-line'), true: tw.color('accent') }} {...props} />
	);
};

type SwitchContainerProps = { title: string; description?: string } & SwitchProps;

export const SwitchContainer: FC<SwitchContainerProps> = ({ title, description, ...props }) => {
	return (
		<View style={tw`flex flex-row items-center justify-between pb-6`}>
			<View style={tw`w-[80%]`}>
				<Text style={tw`text-ink text-sm font-medium`}>{title}</Text>
				{description && <Text style={tw`text-ink-dull mt-2 text-sm`}>{description}</Text>}
			</View>
			<Switch trackColor={{ false: tw.color('app-line'), true: tw.color('accent') }} {...props} />
		</View>
	);
};
