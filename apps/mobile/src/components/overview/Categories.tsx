import { AlphaRSPCError } from '@oscartbeaumont-sd/rspc-client/v2';
import { UseQueryResult } from '@tanstack/react-query';
import React from 'react';
import { FlatList, Pressable, ScrollView, Text, View } from 'react-native';
import { formatNumber, KindStatistics } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { Icon, IconName } from '../icons/Icon';
import Fade from '../layout/Fade';

interface Props {
	kinds: UseQueryResult<KindStatistics, AlphaRSPCError>;
}

const Categories = ({ kinds }: Props) => {
	return (
		<View>
			<Text style={tw`px-7 pb-5 text-lg font-bold text-white`}>Categories</Text>
			<View>
				<Fade color="mobile-screen" width={30} height="100%">
					{/*/
						This addresses the issue of 'Virtualized lists being nested error message'
						by using a scrollview with a different orientation and making it handle scrolling
					*/}
					<ScrollView
						horizontal
						showsHorizontalScrollIndicator={false}
						alwaysBounceVertical={false}
					>
						<FlatList
							data={kinds.data?.statistics
								?.sort((a, b) => b.count - a.count)
								.filter((i) => i.kind !== 0)}
							contentContainerStyle={tw`pl-7 pr-14`}
							numColumns={Math.ceil(Number(kinds.data?.statistics.length ?? 0) / 2)}
							key={kinds.data?.statistics ? 'kinds' : '_'} //needed to update numColumns when data is available
							keyExtractor={(item) => item.name}
							scrollEnabled={false}
							ItemSeparatorComponent={() => <View style={tw`h-3 w-3`} />}
							showsHorizontalScrollIndicator={false}
							renderItem={({ item }) => {
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
									<View style={twStyle('w-[140px]')}>
										<KindItem
											kind={kind}
											name={name}
											icon={icon}
											items={count}
										/>
									</View>
								);
							}}
						/>
					</ScrollView>
				</Fade>
			</View>
		</View>
	);
};

interface KindItemProps {
	kind: number;
	name: string;
	items: number;
	icon: IconName;
	selected?: boolean;
	onClick?: () => void;
	disabled?: boolean;
}

const KindItem = ({ name, icon, items }: KindItemProps) => {
	return (
		<Pressable
			onPress={() => {
				//TODO: implement
			}}
		>
			<View style={twStyle('shrink-0 flex-row items-center', 'gap-2 rounded-lg text-sm')}>
				<Icon name={icon} size={40} style={tw`mr-3 h-12 w-12`} />
				<View>
					<Text style={tw`text-sm font-medium text-ink`}>{name}</Text>
					{items !== undefined && (
						<Text style={tw`text-xs text-ink-faint`}>
							{formatNumber(items)} Item{(items > 1 || items === 0) && 's'}
						</Text>
					)}
				</View>
			</View>
		</Pressable>
	);
};

export default Categories;
