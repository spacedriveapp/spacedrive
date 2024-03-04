import { AnimatePresence, MotiView } from 'moti';
import { Plus, Trash } from 'phosphor-react-native';
import { Pressable, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import { tw } from '~/lib/tailwind';
import { getSearchStore, useSearchStore } from '~/stores/searchStore';

import { Input } from '../form/Input';
import SectionTitle from '../layout/SectionTitle';

export const Extension = () => {
	const searchStore = useSearchStore();
	return (
		<MotiView
			layout={LinearTransition.duration(300)}
			from={{ opacity: 0, translateY: 20 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
			exit={{ opacity: 0 }}
			style={tw`px-6`}
		>
			<SectionTitle style="pb-3" title="Extensions" sub="Search by extensions" />
			<AnimatePresence>
				{searchStore.filters.extension.map((_, index) => (
					<ExtensionInput index={index} key={index} />
				))}
			</AnimatePresence>
			<Pressable onPress={() => getSearchStore().addInput('extension')}>
				<View
					style={tw`flex-row items-center justify-center py-2 border rounded-md border-app-line/50 bg-app-box/50`}
				>
					<Plus size={16} color={tw.color('ink')} />
				</View>
			</Pressable>
		</MotiView>
	);
};

interface NameInputProps {
	index: number;
}

const ExtensionInput = ({ index }: NameInputProps) => {
	const indexSearchStore = useSearchStore().filters.extension;
	return (
		<MotiView
			layout={LinearTransition.duration(300)}
			from={{ opacity: 0, translateY: 10 }}
			animate={{ opacity: 1, translateY: 0 }}
			transition={{ type: 'timing', duration: 300 }}
			style={tw`flex-row gap-2 mb-2`}
		>
			<Input
				variant="default"
				style={tw`flex-1`}
				size="md"
				value={indexSearchStore[index] as string}
				onChangeText={(text) => getSearchStore().setInput(index, text, 'extension')}
				placeholder="Extension..."
			/>
			{index !== 0 && (
				<Pressable
					onPress={() => getSearchStore().removeInput(index, 'extension')}
					style={tw`items-center justify-center p-2 border rounded-md border-app-line bg-app-input`}
				>
					<View>
						<Trash size={20} color={tw.color('ink')} />
					</View>
				</Pressable>
			)}
		</MotiView>
	);
};
