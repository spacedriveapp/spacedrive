import { MagnifyingGlass } from 'phosphor-react-native';
import { useEffect } from 'react';
import { TextInput, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { getSearchStore } from '~/stores/searchStore';

interface Props {
	placeholder: string;
}

export default function Search({ placeholder }: Props) {
	const searchStore = getSearchStore();
	// Clear search when unmounting
	useEffect(() => {
		return () => {
			searchStore.setSearch('');
		};
	}, [searchStore]);
	return (
		<View
			style={tw`flex flex-row items-center justify-between w-full h-auto px-3 py-2 mt-3 border rounded-md shadow-sm border-app-inputborder bg-app-input`}
		>
			<TextInput
				onChangeText={(text) => searchStore.setSearch(text)}
				placeholderTextColor={tw.color('text-ink-dull')}
				style={tw`w-[90%] text-white text-sm leading-0`}
				placeholder={placeholder}
			/>
			<MagnifyingGlass size={20} weight="bold" color={tw.color('text-ink-dull')} />
		</View>
	);
}
