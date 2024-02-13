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
			style={tw`mt-4 flex h-11 w-full flex-row items-center justify-between rounded-md border border-sidebar-line bg-sidebar-button px-3 shadow-sm`}
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
