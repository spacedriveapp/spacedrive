import { useNavigation } from '@react-navigation/native';
import { Plus } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchStackScreenProps } from '~/navigation/SearchStack';
import { useSearchStore } from '~/stores/searchStore';

const SaveAdd = () => {
	const searchStore = useSearchStore();
	const navigation = useNavigation<SearchStackScreenProps<'Home'>['navigation']>();
	return (
		<View
			style={twStyle(
				`h-[100px] flex-row justify-between gap-2 border-t border-app-line/50 bg-mobile-header px-6 pt-5`,
				{
					position: 'fixed'
				}
			)}
		>
			<Button
				disabled={searchStore.disableActionButtons}
				style={twStyle(`h-10 flex-1 flex-row gap-1`, {
					opacity: searchStore.disableActionButtons ? 0.5 : 1
				})}
				variant="dashed"
			>
				<Plus weight="bold" size={12} color={tw.color('text-ink-dull')} />
				<Text style={tw`font-medium text-ink-dull`}>Save search</Text>
			</Button>
			<Button
				disabled={searchStore.disableActionButtons}
				style={twStyle(`h-10 flex-1 flex-row gap-1`, {
					opacity: searchStore.disableActionButtons ? 0.5 : 1
				})}
				variant="accent"
				onPress={() => {
					searchStore.applyFilters();
					navigation.navigate('Home');
				}}
			>
				<Plus weight="bold" size={12} color={tw.color('white')} />
				<Text style={tw`font-medium text-ink`}>Add filters</Text>
			</Button>
		</View>
	);
};

export default SaveAdd;
