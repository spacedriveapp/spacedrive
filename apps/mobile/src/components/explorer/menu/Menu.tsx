import { AnimatePresence, MotiView } from 'moti';
import { MonitorPlay, Rows, SquaresFour } from 'phosphor-react-native';
import { Pressable, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';

import SortByMenu from './SortByMenu';

const Menu = () => {
	const store = useExplorerStore();

	return (
		<AnimatePresence>
			{store.toggleMenu && (
				<MotiView
					from={{ translateY: -70 }}
					animate={{ translateY: 0 }}
					exit={{ translateY: -70 }}
					transition={{
						type: 'timing',
						duration: 300,
						repeat: 0
					}}
					style={tw`w-screen flex-row items-center justify-between border-b border-app-cardborder bg-app-header px-5 py-3`}
				>
					<SortByMenu />
					<View style={tw`flex-row gap-3`}>
						<Pressable
							hitSlop={12}
							onPress={() => (getExplorerStore().layoutMode = 'grid')}
						>
							<SquaresFour
								weight={'regular'}
								color={tw.color(
									store.layoutMode === 'grid' ? 'accent' : 'text-ink-faint'
								)}
								size={23}
							/>
						</Pressable>
						<Pressable
							hitSlop={12}
							onPress={() => (getExplorerStore().layoutMode = 'list')}
						>
							<Rows
								weight={'regular'}
								color={tw.color(
									store.layoutMode === 'list' ? 'accent' : 'text-ink-faint'
								)}
								size={23}
							/>
						</Pressable>
						<Pressable
							hitSlop={12}
							onPress={() => (getExplorerStore().layoutMode = 'media')}
						>
							<MonitorPlay
								weight={'regular'}
								color={tw.color(
									store.layoutMode === 'media' ? 'accent' : 'text-ink-faint'
								)}
								size={23}
							/>
						</Pressable>
					</View>
				</MotiView>
			)}
		</AnimatePresence>
	);
};
export default Menu;
