import { IconTypes } from '@sd/assets/util';
import { ObjectKind } from '@sd/client';
import { MotiView } from 'moti';
import { memo, useCallback, useMemo } from 'react';
import { FlatList, Pressable, Text, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import { Icon } from '~/components/icons/Icon';
import Card from '~/components/layout/Card';
import SectionTitle from '~/components/layout/SectionTitle';
import VirtualizedListWrapper from '~/components/layout/VirtualizedListWrapper';
import { tw, twStyle } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

export const kinds = Object.keys(ObjectKind)
	.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
	.map((key) => {
		const kind = ObjectKind[Number(key)];
		return {
			name: kind as string,
			value: Number(key),
			icon: (kind + '20') as IconTypes
		};
	});

const Kind = () => {
	const searchStore = useSearchStore();

	return (
		<MotiView
			layout={LinearTransition.duration(300)}
			from={{ opacity: 0, translateY: 20 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
			exit={{ opacity: 0 }}
		>
			<SectionTitle
				style="px-6 pb-3"
				title="Kind"
				sub="What kind of objects should be searched?"
			/>
			<View>
					<VirtualizedListWrapper horizontal>
						<FlatList
							data={kinds}
							renderItem={({ item }) => <KindFilter data={item} />}
							contentContainerStyle={tw`px-6`}
							numColumns={kinds && Math.ceil(Number(kinds.length) / 2)}
							key={kinds ? 'kindsSearch' : '_'}
							scrollEnabled={false}
							extraData={searchStore.filters.kind}
							ItemSeparatorComponent={() => <View style={tw`h-2 w-2`} />}
							keyExtractor={(item) => item.value.toString()}
							showsHorizontalScrollIndicator={false}
							style={tw`flex-row`}
						/>
					</VirtualizedListWrapper>
			</View>
		</MotiView>
	);
};

interface KindFilterProps {
	data: {
		name: string;
		value: number;
		icon: IconTypes;
	};
}

const KindFilter = memo(({ data }: KindFilterProps) => {
	const searchStore = useSearchStore();

	const isSelected = useMemo(
		() => searchStore.filters.kind.some((v) => v.name === data.name),
		[searchStore.filters.kind, data.name]
	);

	const onPress = useCallback(() => {
		searchStore.updateFilters('kind', {
			id: data.value,
			name: data.name,
			icon: data.icon
		});
	}, [searchStore, data]);

	return (
		<Pressable onPress={onPress}>
			<Card
				style={twStyle(`mr-2 w-auto flex-row items-center gap-2 p-2.5`, {
					borderColor: isSelected ? tw.color('accent') : tw.color('app-cardborder')
				})}
			>
				<Icon name={data.icon} size={20} />
				<Text style={tw`text-sm text-ink`}>{data.name}</Text>
			</Card>
		</Pressable>
	);
});

export default Kind;
