import { Text, View } from 'react-native';
import { ClassInput } from 'twrnc';
import { twStyle } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';

interface Props {
	description: string; //description of empty state
	icon: IconName; //Spacedrive icon
	style?: ClassInput; //Tailwind classes
	iconSize?: number; //Size of the icon
	textSize?: ClassInput; //Size of the text
}

const Empty = ({ description, icon, style, textSize = 'text-sm', iconSize = 38 }: Props) => {
	return (
		<View
			style={twStyle(
				`relative mx-auto h-auto w-full flex-col items-center justify-center overflow-hidden
			 rounded-md border border-dashed border-sidebar-line p-4`,
				style
			)}
		>
			<Icon name={icon} size={iconSize} />
			<Text style={twStyle(`mt-2 text-center font-medium text-ink-dull`, textSize)}>
				{description}
			</Text>
		</View>
	);
};

export default Empty;
