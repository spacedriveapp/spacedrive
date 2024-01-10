import { MagnifyingGlass } from 'phosphor-react-native';
import { TextInput, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { getSearchStore } from '~/stores/searchStore';

interface Props {
	placeholder: string;
}

export default function Search({ placeholder }: Props) {
	const searchStore = getSearchStore();
	return (
		<View
			style={tw`flex flex-row items-center justify-between w-full px-3 mt-4 border rounded-md shadow-sm h-11 border-sidebar-line bg-sidebar-button`}
		>
			<TextInput
				onChangeText={(text) => searchStore.setSearch(text)}
				placeholderTextColor={tw.color('text-ink-dull')}
				style={tw`w-[90%] text-white`}
				placeholder={placeholder}
			/>
			<MagnifyingGlass size={20} weight="bold" color={tw.color('text-ink-dull')} />
		</View>
	);
}
