import { useNavigation } from '@react-navigation/native';
import { Pressable, Text, View } from 'react-native';
import { ClassInput } from 'twrnc';
import { formatNumber } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

import { Icon, IconName } from '../icons/Icon';

interface CategoryItemProps {
	kind: number;
	name: string;
	items: bigint | number;
	icon: IconName;
	selected?: boolean;
	onClick?: () => void;
	disabled?: boolean;
	style?: ClassInput;
}

const CategoryItem = ({ name, icon, items, style, kind }: CategoryItemProps) => {
	const navigation = useNavigation();
	const searchStore = useSearchStore();
	return (
		<Pressable
			style={twStyle(
				'w-[31.7%] flex-col items-center border border-app-cardborder bg-app-card p-2',
				'gap-1.5 rounded-lg text-sm',
				style
			)}
			onPress={() => {
				searchStore.updateFilters(
					'kind',
					{
						name,
						icon: (icon + '20') as IconName,
						id: kind
					},
					true
				);
				navigation.navigate('SearchStack', {
					screen: 'Search'
				});
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
