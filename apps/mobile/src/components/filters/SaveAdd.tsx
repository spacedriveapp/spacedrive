import { AnimatePresence, MotiView } from 'moti';
import { useMemo } from 'react';
import { Text, View } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchFilters, useSearchStore } from '~/stores/searchStore';

export const SaveAdd = () => {
	const searchStore = useSearchStore();
	const filterCount = useMemo(() => {
		let count = 0;
		for (const filter in searchStore.filters) {
			if (searchStore.filters[filter as SearchFilters].some((value) => value !== '')) {
				count++;
			}
		}
		return count;
	}, [searchStore.filters]);
	return (
		<View
			style={twStyle(
				`flex-row justify-between gap-2 border-t border-app-line/50 h-[100px] pt-5 px-6 bg-mobile-header`,
				{
					position: 'fixed'
				}
			)}
		>
			<Button
				disabled={filterCount === 0}
				style={twStyle(`flex-1 h-10`, filterCount === 0 && 'opacity-50')}
				variant="dashed"
			>
				<Text style={tw`font-bold text-ink-dull`}>+ Save search</Text>
			</Button>
			<Button
				disabled={filterCount === 0}
				style={twStyle(`flex-1 h-10`, filterCount === 0 && 'opacity-50')}
				variant="accent"
			>
				<Text style={tw`font-bold text-ink`}>+ Add filters</Text>
			</Button>
		</View>
	);
};
