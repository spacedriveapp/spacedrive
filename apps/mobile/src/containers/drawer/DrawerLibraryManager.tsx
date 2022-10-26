import { useDrawerStatus } from '@react-navigation/drawer';
import { MotiView } from 'moti';
import { CaretRight, Gear, Lock, Plus } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { useLibraryStore } from '~/stores/libraryStore';

import { AnimatedHeight } from '../../components/animation/layout';
import Divider from '../../components/primitive/Divider';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';

const DrawerLibraryManager = () => {
	const [dropdownClosed, setDropdownClosed] = useState(true);

	// Closes the dropdown when the drawer is closed
	const isDrawerOpen = useDrawerStatus() === 'open';
	useEffect(() => {
		if (!isDrawerOpen) setDropdownClosed(true);
	}, [isDrawerOpen]);

	const { currentLibrary, libraries, switchLibrary } = useLibraryStore();

	return (
		<View>
			<Pressable onPress={() => setDropdownClosed((v) => !v)}>
				<View
					style={tw.style(
						'flex flex-row justify-between items-center px-3 h-10 w-full bg-gray-500 border border-[#333949] bg-opacity-40 shadow-sm',
						dropdownClosed ? 'rounded' : 'rounded-t border-b-gray-550'
					)}
				>
					<Text style={tw`text-gray-200 text-sm font-semibold`}>{currentLibrary?.config.name}</Text>
					<MotiView
						animate={{
							rotateZ: dropdownClosed ? '0deg' : '90deg'
						}}
						transition={{ type: 'timing' }}
					>
						<CaretRight color={tw.color('text-gray-200')} size={16} style={tw`ml-2`} />
					</MotiView>
				</View>
			</Pressable>
			<AnimatedHeight hide={dropdownClosed}>
				<View
					style={tw`py-2 px-2 bg-gray-500 border-l border-b border-r border-[#333949] bg-opacity-40 rounded-b`}
				>
					{/* Libraries */}
					{libraries?.map((library) => (
						<Pressable key={library.uuid} onPress={() => switchLibrary(library.uuid)}>
							<View
								style={tw.style(
									'p-2',
									library.uuid === currentLibrary.uuid && 'bg-gray-500 bg-opacity-70 rounded'
								)}
							>
								<Text style={tw`text-sm text-gray-200 font-semibold`}>{library.config.name}</Text>
							</View>
						</Pressable>
					))}
					<Divider style={tw`mt-2 mb-2`} />
					{/* Menu */}
					<Pressable onPress={() => console.log('settings')}>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Gear size={16} color={tw.color('gray-100')} style={tw`mr-2`} />
							<Text style={tw`text-sm text-gray-200 font-semibold`}>Library Settings</Text>
						</View>
					</Pressable>
					{/* Create Library */}
					<CreateLibraryDialog>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Plus size={16} weight="bold" color={tw.color('gray-100')} style={tw`mr-2`} />
							<Text style={tw`text-sm text-gray-200 font-semibold`}>Add Library</Text>
						</View>
					</CreateLibraryDialog>
					<Pressable onPress={() => console.log('lock')}>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Lock size={16} weight="bold" color={tw.color('gray-100')} style={tw`mr-2`} />
							<Text style={tw`text-sm text-gray-200 font-semibold`}>Lock</Text>
						</View>
					</Pressable>
				</View>
			</AnimatedHeight>
		</View>
	);
};

export default DrawerLibraryManager;
