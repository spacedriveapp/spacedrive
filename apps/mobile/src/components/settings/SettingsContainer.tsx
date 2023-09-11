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
			{title && (
				<Text style={tw`pb-2 pl-3 text-sm font-semibold text-ink-dull`}>{title}</Text>
			)}
			{children}
			{description && <Text style={tw`px-3 pt-2 text-sm text-ink-dull`}>{description}</Text>}
		</View>
	);
}

export const SettingsTitle = styled(Text, 'text-ink mb-1.5 ml-1 text-sm font-medium');
export const SettingsInputInfo = styled(Text, 'mt-2 text-xs text-ink-faint');
