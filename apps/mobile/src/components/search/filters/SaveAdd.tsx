import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import { useEffect, useRef } from 'react';
import { Platform, Text, View } from 'react-native';
import { ModalRef } from '~/components/layout/Modal';
import SaveSearchModal from '~/components/modal/search/SaveSearchModal';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { getSearchStore, useSearchStore } from '~/stores/searchStore';

const SaveAdd = () => {
	const searchStore = useSearchStore();
	const navigation = useNavigation<SearchStackScreenProps<'Search'>['navigation']>();
	const modalRef = useRef<ModalRef>(null);

	const filtersApplied = Object.keys(searchStore.appliedFilters).length > 0;
	const buttonDisable = !filtersApplied && searchStore.disableActionButtons;
	const isAndroid = Platform.OS === 'android';

	// enable action buttons if any filter value is present
	useEffect(() => {
		const hasNonEmptyFilter = Object.values(searchStore.filters)
			.flat()
			.some((v) => v !== '' && v !== false);
		getSearchStore().disableActionButtons = !hasNonEmptyFilter;
	}, [searchStore.filters]);

	return (
		<View
			style={twStyle(
				`flex-row items-center justify-between gap-2 border-t border-app-cardborder bg-app-header px-6`,
				isAndroid ? 'py-6' : 'pb-10 pt-7',
				{
					position: 'fixed' // tw doesn't support fixed
				}
			)}
		>
			<Button
				disabled={buttonDisable}
				style={twStyle(`h-10 flex-1 flex-row gap-1`, {
					opacity: buttonDisable ? 0.5 : 1
				})}
				variant="dashed"
				onPress={() => modalRef.current?.present()}
			>
				<Plus weight="bold" size={12} color={tw.color('text-ink-dull')} />
				<Text style={tw`font-medium text-ink-dull`}>Save search</Text>
			</Button>
			<Button
				disabled={buttonDisable}
				style={twStyle(`h-10 flex-1 flex-row gap-1`, {
					opacity: buttonDisable ? 0.5 : 1
				})}
				variant="accent"
				onPress={() => {
					searchStore.applyFilters();
					navigation.navigate('Search');
				}}
			>
				<Plus weight="bold" size={12} color={tw.color('white')} />
				<Text style={tw`font-medium text-ink`}>
					{filtersApplied ? 'Update filters' : 'Add filters'}
				</Text>
			</Button>
			<SaveSearchModal ref={modalRef} />
		</View>
	);
};

export default SaveAdd;
