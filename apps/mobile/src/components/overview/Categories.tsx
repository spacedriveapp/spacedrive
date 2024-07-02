import { useNavigation } from '@react-navigation/native';
import { DotsThree } from 'phosphor-react-native';
import React from 'react';
import { Text, View } from 'react-native';
import { uint32ArrayToBigInt, useLibraryQuery } from '@sd/client';
import { tw } from '~/lib/tailwind';
import { OverviewStackScreenProps } from '~/navigation/tabs/OverviewStack';

import { IconName } from '../icons/Icon';
import { Button } from '../primitive/Button';
import CategoryItem from './CategoryItem';

export default function CategoriesScreen() {
	const kinds = useLibraryQuery(['library.kindStatistics']);
	const navigation = useNavigation<OverviewStackScreenProps<'Overview'>['navigation']>();
	return (
		<View style={tw`px-5`}>
			<View style={tw`flex-row items-center justify-between pb-5`}>
				<Text style={tw`text-lg font-bold text-white`}>Categories</Text>
				<Button
					onPress={() => {
						navigation.navigate('Categories');
					}}
					style={tw`h-8 w-8 rounded-full`}
					variant="gray"
				>
					<DotsThree weight="bold" size={18} color={'white'} />
				</Button>
			</View>
			<View style={tw`flex-row flex-wrap gap-2`}>
				{kinds.data?.statistics
					?.sort((a, b) => {
						const aCount = Number(a.count);
						const bCount = Number(b.count);
						if (aCount === bCount) return 0;
						return aCount > bCount ? -1 : 1;
					})
					.filter((i) => i.kind !== 0)
					.slice(0, 6)
					.map((item) => {
						const { kind, name, count } = item;
						let icon = name as IconName;
						switch (name) {
							case 'Code':
								icon = 'Terminal';
								break;
							case 'Unknown':
								icon = 'Undefined';
								break;
						}
						return (
							<CategoryItem
								key={name}
								kind={kind}
								name={name}
								icon={icon}
								items={Number(count)}
							/>
						);
					})}
			</View>
		</View>
	);
}
