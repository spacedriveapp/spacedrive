import { PropsWithChildren } from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';

type SettingsContainerProps = PropsWithChildren<{
	title?: string;
	description?: string;
}>;

export function SettingsContainer({ children, title, description }: SettingsContainerProps) {
	return (
		<View>
			{title && <Text style={tw`text-ink-dull pb-2 pl-3 text-sm font-semibold`}>{title}</Text>}
			{children}
			{description && <Text style={tw`text-ink-dull px-3 pt-2 text-sm`}>{description}</Text>}
		</View>
	);
}
