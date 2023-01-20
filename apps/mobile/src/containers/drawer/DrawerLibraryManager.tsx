import { useDrawerStatus } from '@react-navigation/drawer';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { CaretDown, Gear, Lock, Plus } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Alert, Pressable, Text, View } from 'react-native';
import { useCurrentLibrary } from '~/../../../packages/client/src';
import tw from '~/lib/tailwind';
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

	const { library: currentLibrary, libraries, switchLibrary } = useCurrentLibrary();

	const navigation = useNavigation();

	return (
		<View>
			<Pressable onPress={() => setDropdownClosed((v) => !v)}>
				<View
					style={tw.style(
						'flex flex-row justify-between items-center px-3 h-10 w-full bg-sidebar-box border shadow-sm',
						dropdownClosed
							? 'rounded-md border-sidebar-line/50'
							: 'rounded-t-md border-b-app-box border-sidebar-line bg-sidebar-button'
					)}
				>
					<Text style={tw`text-ink text-sm font-semibold`}>{currentLibrary?.config.name}</Text>
					<MotiView
						animate={{
							rotate: dropdownClosed ? '0deg' : '180deg',
							translateX: dropdownClosed ? 0 : -9
						}}
						transition={{ type: 'timing', duration: 100 }}
					>
						<CaretDown color="white" size={18} weight="bold" style={tw`ml-2`} />
					</MotiView>
				</View>
			</Pressable>
			<AnimatedHeight hide={dropdownClosed}>
				<View
					style={tw`py-2 px-2 bg-sidebar-button border-l border-b border-r border-sidebar-line rounded-b-md`}
				>
					{/* Libraries */}
					{libraries?.map((library) => (
						<Pressable key={library.uuid} onPress={() => switchLibrary(library.uuid)}>
							<View
								style={tw.style(
									'p-2 mt-1',
									currentLibrary.uuid === library.uuid && 'bg-accent rounded'
								)}
							>
								<Text
									style={tw.style(
										'text-sm text-ink font-semibold',
										currentLibrary.uuid === library.uuid && 'text-white'
									)}
								>
									{library.config.name}
								</Text>
							</View>
						</Pressable>
					))}
					<Divider style={tw`mt-2 mb-2`} />
					{/* Menu */}
					{/* Create Library */}
					<CreateLibraryDialog>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Plus size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm text-white font-semibold`}>New Library</Text>
						</View>
					</CreateLibraryDialog>
					{/* Manage Library */}
					<Pressable
						onPress={() => navigation.navigate('Settings', { screen: 'LibraryGeneralSettings' })}
					>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Gear size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm text-white font-semibold`}>Manage Library</Text>
						</View>
					</Pressable>
					{/* Lock */}
					<Pressable onPress={() => Alert.alert('TODO')}>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Lock size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm text-white font-semibold`}>Lock</Text>
						</View>
					</Pressable>
				</View>
			</AnimatedHeight>
		</View>
	);
};

export default DrawerLibraryManager;
