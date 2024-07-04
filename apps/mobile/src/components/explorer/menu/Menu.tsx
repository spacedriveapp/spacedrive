import { AnimatePresence, MotiView } from 'moti';
import { Rows, SquaresFour } from 'phosphor-react-native';
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
						{store.layoutMode === 'grid' ? (
							<Pressable
								hitSlop={12}
								onPress={() => (getExplorerStore().layoutMode = 'list')}
							>
								<Rows weight="fill" color={tw.color('text-ink-faint')} size={23} />
							</Pressable>
						) : (
							<Pressable
								hitSlop={12}
								onPress={() => (getExplorerStore().layoutMode = 'grid')}
							>
								<SquaresFour
									weight="fill"
									color={tw.color('text-ink-faint')}
									size={23}
								/>
							</Pressable>
						)}
						{/* <Pressable
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
							</Pressable> */}
					</View>
				</MotiView>
			)}
		</AnimatePresence>
	);
};
export default Menu;
