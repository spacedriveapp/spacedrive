import { AnimatePresence, MotiView } from 'moti';
import { Plus, Trash } from 'phosphor-react-native';
import { Pressable, View } from 'react-native';
import { LinearTransition } from 'react-native-reanimated';
import SectionTitle from '~/components/layout/SectionTitle';
import { Input } from '~/components/primitive/Input';
import { tw } from '~/lib/tailwind';
import { useSearchStore } from '~/stores/searchStore';

const Name = () => {
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
			<SectionTitle style="pb-3" title="Name" sub="Search by names" />
			<AnimatePresence>
				{searchStore.filters.name.map((_, index) => (
					<NameInput index={index} key={index} />
				))}
			</AnimatePresence>
			<Pressable onPress={() => searchStore.addInput('name')}>
				<View
					style={tw`flex-row items-center justify-center rounded-md border border-app-line/50 bg-app-box/50 py-2`}
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

const NameInput = ({ index }: NameInputProps) => {
	const searchStore = useSearchStore();
	const indexNameSearch = searchStore.filters.name[index];
	return (
		<View style={tw`mb-2 flex-row gap-2`}>
			<Input
				variant="default"
				style={tw`flex-1`}
				size="md"
				value={indexNameSearch as string}
				onChangeText={(text) => searchStore.setInput(index, text, 'name')}
				placeholder="Name..."
			/>
			{index !== 0 && (
				<Pressable
					onPress={() => searchStore.removeInput(index, 'name')}
					style={tw`items-center justify-center rounded-md border border-app-cardborder bg-app-boxLight p-2`}
				>
					<Trash size={20} color={tw.color('ink')} />
				</Pressable>
			)}
		</View>
	);
};

export default Name;
