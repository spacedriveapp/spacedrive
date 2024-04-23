import { AnimatePresence, MotiView } from 'moti';
import { MonitorPlay, Rows, SlidersHorizontal, SquaresFour } from 'phosphor-react-native';
import { Pressable, View } from 'react-native';
import { toast } from '~/components/primitive/Toast';
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
					transition={{
						type: 'timing',
						duration: 300,
						repeat: 0,
						repeatReverse: false
					}}
					exit={{ translateY: -70 }}
				>
					<View
						style={tw`w-screen flex-row items-center justify-between border-b border-app-cardborder bg-app-header px-7 py-4`}
					>
						<View style={tw`flex-row gap-3`}>
							<Pressable onPress={() => (getExplorerStore().layoutMode = 'grid')}>
								<SquaresFour
									color={tw.color(
										store.layoutMode === 'grid'
											? 'text-accent'
											: 'text-ink-dull'
									)}
									size={23}
								/>
							</Pressable>
							<Pressable onPress={() => (getExplorerStore().layoutMode = 'list')}>
								<Rows
									color={tw.color(
										store.layoutMode === 'list'
											? 'text-accent'
											: 'text-ink-dull'
									)}
									size={23}
								/>
							</Pressable>
							<Pressable
								onPress={() => toast.error('Media view is not available yet...')}
								// onPress={() => (getExplorerStore().layoutMode = 'media')}
							>
								<MonitorPlay
									color={tw.color(
										store.layoutMode === 'media'
											? 'text-accent'
											: 'text-ink-dull'
									)}
									size={23}
								/>
							</Pressable>
						</View>
						<SortByMenu />
					</View>
				</MotiView>
			)}
		</AnimatePresence>
	);
};
export default Menu;
