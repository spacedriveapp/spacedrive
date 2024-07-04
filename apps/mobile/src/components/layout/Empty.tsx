import { Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { ClassInput } from 'twrnc';
import { twStyle } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';

interface Props {
	description: string; //description of empty state
	icon?: IconName; //Spacedrive icon
	style?: ClassInput; //Tailwind classes
	iconSize?: number; //Size of the icon
	textStyle?: ClassInput; //Size of the text
	includeHeaderHeight?: boolean; //Height of the header
}

const Empty = ({
	description,
	icon,
	style,
	includeHeaderHeight = false,
	textStyle,
	iconSize = 38
}: Props) => {
	const headerHeight = useSafeAreaInsets().top;
	return (
		<View
			style={twStyle(
				`relative mx-auto h-auto w-full flex-col items-center justify-center overflow-hidden
			 rounded-md border border-dashed border-sidebar-line p-4`,
				{ marginBottom: includeHeaderHeight ? headerHeight : 0 },
				style
			)}
		>
			{icon && <Icon name={icon} size={iconSize} />}
			<Text style={twStyle(`mt-2 text-center text-sm font-medium text-ink-dull`, textStyle)}>
				{description}
			</Text>
		</View>
	);
};

export default Empty;
