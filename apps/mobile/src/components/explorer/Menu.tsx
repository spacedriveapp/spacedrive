import { MonitorPlay, Rows, SlidersHorizontal, SquaresFour } from 'phosphor-react-native';
import { Pressable, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { ExplorerLayoutMode } from '~/stores/explorerStore';

interface MenuProps {
	layoutMode: ExplorerLayoutMode;
	changeLayoutMode: (kind: ExplorerLayoutMode) => void;
}

const Menu = ({ layoutMode, changeLayoutMode }: MenuProps) => {
	return (
		<View
			style={tw`w-screen flex-row justify-between border-b border-app-cardborder bg-app-header px-7 py-4`}
		>
			<View style={tw`flex-row gap-3`}>
				<Pressable onPress={() => changeLayoutMode('grid')}>
					<SquaresFour
						color={tw.color(layoutMode === 'grid' ? 'text-accent' : 'text-ink-dull')}
						size={23}
					/>
				</Pressable>
				<Pressable onPress={() => changeLayoutMode('list')}>
					<Rows
						color={tw.color(layoutMode === 'list' ? 'text-accent' : 'text-ink-dull')}
						size={23}
					/>
				</Pressable>
				<Pressable onPress={() => changeLayoutMode('media')}>
					<MonitorPlay
						color={tw.color(layoutMode === 'media' ? 'text-accent' : 'text-ink-dull')}
						size={23}
					/>
				</Pressable>
			</View>
			<SlidersHorizontal style={tw`text-ink-dull`} />
		</View>
	);
};

export default Menu;
