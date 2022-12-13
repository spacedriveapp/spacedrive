import { PropsWithChildren } from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';

type SettingsContainerProps = PropsWithChildren<{
	title?: string;
	description?: string;
}>;

export function SettingsContainer({ children, title, description }: SettingsContainerProps) {
	return (
		<View style={tw``}>
			{title && <Text style={tw`pb-2 pl-3 text-sm font-semibold text-ink-dull`}>{title}</Text>}
			{children}
			{description && <Text style={tw`text-ink-dull text-sm px-3 pt-2`}>{description}</Text>}
		</View>
	);
}
