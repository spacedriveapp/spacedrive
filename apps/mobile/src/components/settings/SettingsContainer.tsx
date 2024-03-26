import { PropsWithChildren } from 'react';
import { Text, View } from 'react-native';
import { styled, tw } from '~/lib/tailwind';

type SettingsContainerProps = PropsWithChildren<{
	title?: string;
	description?: string;
}>;

export function SettingsContainer({ children, title, description }: SettingsContainerProps) {
	return (
		<View>
			{title && <Text style={tw`pb-2 text-sm font-semibold text-ink-dull`}>{title}</Text>}
			{children}
			{description && <Text style={tw`pt-2 text-sm text-ink-dull`}>{description}</Text>}
		</View>
	);
}

export const SettingsTitle = styled(Text, 'text-ink text-sm font-medium');
export const SettingsInputInfo = styled(Text, 'mt-2 text-xs text-ink-faint');
