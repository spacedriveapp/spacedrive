import { IconTypes } from '@sd/assets/util';
import { MotiView } from 'moti';
import { MotiPressable } from 'moti/interactions';
import { FlatList, Text, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import { ObjectKind } from '@sd/client';
import { tw } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import SectionTitle from '../layout/SectionTitle';
import VirtualizedListWrapper from '../layout/VirtualizedListWrapper';

const kinds = Object.keys(ObjectKind)
	.filter((key) => !isNaN(Number(key)) && ObjectKind[Number(key)] !== undefined)
	.map((key) => {
		const kind = ObjectKind[Number(key)];
		return {
			name: kind as string,
			value: Number(key),
			icon: (kind + '20') as IconTypes
		};
	});

export const Kind = () => {
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
				<Fade color="mobile-screen" width={30} height="100%">
					<VirtualizedListWrapper horizontal>
						<FlatList
							data={kinds}
							renderItem={({ item }) => <KindFilter data={item} />}
							contentContainerStyle={tw`pl-6`}
							numColumns={kinds && Math.ceil(Number(kinds.length) / 2)}
							key={kinds ? 'kindsSearch' : '_'}
							scrollEnabled={false}
							ItemSeparatorComponent={() => <View style={tw`h-2 w-2`} />}
							keyExtractor={(item) => item.value.toString()}
							showsHorizontalScrollIndicator={false}
							style={tw`flex-row`}
						/>
					</VirtualizedListWrapper>
				</Fade>
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

const KindFilter = ({ data }: KindFilterProps) => {
	const searchStore = useSearchStore();
	const isSelected = searchStore.filters.kind.some((v) => v.name === data.name);
	return (
		<MotiPressable
			onPress={() =>
				searchStore.updateFilters('kind', {
					id: data.value,
					name: data.name
				})
			}
			animate={{
				borderColor: isSelected ? tw.color('accent') : tw.color('app-line/50')
			}}
			style={tw`mr-2 w-auto flex-row items-center gap-2 rounded-md border border-app-line/50 bg-app-box/50 p-2.5`}
		>
			<Icon name={data.icon} size={20} />
			<Text style={tw`text-sm text-ink`}>{data.name}</Text>
		</MotiPressable>
	);
};
