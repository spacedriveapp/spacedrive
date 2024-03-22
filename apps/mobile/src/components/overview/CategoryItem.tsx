import { Pressable, Text, View } from 'react-native';
import { ClassInput } from 'twrnc';
import { formatNumber } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';

interface CategoryItemProps {
	kind: number;
	name: string;
	items: number;
	icon: IconName;
	selected?: boolean;
	onClick?: () => void;
	disabled?: boolean;
	style?: ClassInput;
}

const CategoryItem = ({ name, icon, items, style }: CategoryItemProps) => {
	return (
		<Pressable
			style={twStyle(
				'w-[31.7%] flex-col items-center border border-app-cardborder bg-app-card p-2',
				'gap-1.5 rounded-lg text-sm',
				style
			)}
			onPress={() => {
				//TODO: implement
			}}
		>
			<Icon name={icon} size={56} />
			<View>
				<Text numberOfLines={1} style={tw`text-center text-sm font-medium text-ink`}>
					{name}
				</Text>
				{items !== undefined && (
					<Text numberOfLines={1} style={tw`text-center text-xs text-ink-faint`}>
						{formatNumber(items)} Item{(items > 1 || items === 0) && 's'}
					</Text>
				)}
			</View>
		</Pressable>
	);
};

export default CategoryItem;
