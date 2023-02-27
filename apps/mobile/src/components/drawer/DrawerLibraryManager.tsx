import { useDrawerStatus } from '@react-navigation/drawer';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { CaretDown, Gear, Lock, Plus } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Alert, Pressable, Text, View } from 'react-native';
import { useClientContext } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { currentLibraryStore } from '~/utils/nav';
import { AnimatedHeight } from '../animation/layout';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import Divider from '../primitive/Divider';

const DrawerLibraryManager = () => {
	const [dropdownClosed, setDropdownClosed] = useState(true);

	// Closes the dropdown when the drawer is closed
	const isDrawerOpen = useDrawerStatus() === 'open';
	useEffect(() => {
		if (!isDrawerOpen) setDropdownClosed(true);
	}, [isDrawerOpen]);

	const { library: currentLibrary, libraries } = useClientContext();

	const navigation = useNavigation();

	return (
		<View>
			<Pressable onPress={() => setDropdownClosed((v) => !v)}>
				<View
					style={twStyle(
						'bg-sidebar-box flex h-10 w-full flex-row items-center justify-between border px-3 shadow-sm',
						dropdownClosed
							? 'border-sidebar-line/50 rounded-md'
							: 'border-b-app-box border-sidebar-line bg-sidebar-button rounded-t-md'
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
				<View style={tw`bg-sidebar-button border-sidebar-line rounded-b-md p-2`}>
					{/* Libraries */}
					{libraries.data?.map((library) => {
						return (
							<Pressable key={library.uuid} onPress={() => (currentLibraryStore.id = library.uuid)}>
								<View
									style={twStyle(
										'mt-1 p-2',
										currentLibrary?.uuid === library.uuid && 'bg-accent rounded'
									)}
								>
									<Text
										style={twStyle(
											'text-ink text-sm font-semibold',
											currentLibrary?.uuid === library.uuid && 'text-white'
										)}
									>
										{library.config.name}
									</Text>
								</View>
							</Pressable>
						);
					})}
					<Divider style={tw`my-2`} />
					{/* Menu */}
					{/* Create Library */}
					<CreateLibraryDialog>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Plus size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm font-semibold text-white`}>New Library</Text>
						</View>
					</CreateLibraryDialog>
					{/* Manage Library */}
					<Pressable
						onPress={() => navigation.navigate('Settings', { screen: 'LibraryGeneralSettings' })}
					>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Gear size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm font-semibold text-white`}>Manage Library</Text>
						</View>
					</Pressable>
					{/* Lock */}
					<Pressable onPress={() => Alert.alert('TODO')}>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Lock size={18} weight="bold" color="white" style={tw`mr-2`} />
							<Text style={tw`text-sm font-semibold text-white`}>Lock</Text>
						</View>
					</Pressable>
				</View>
			</AnimatedHeight>
		</View>
	);
};

export default DrawerLibraryManager;
