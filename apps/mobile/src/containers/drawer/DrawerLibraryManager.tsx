import { useDrawerStatus } from '@react-navigation/drawer';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { CaretRight, Gear, Lock, Plus } from 'phosphor-react-native';
import { useEffect, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { useCurrentLibrary } from '~/../../../packages/client/src';
import tw, { twStyle } from '~/lib/tailwind';
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
					style={twStyle(
						'border-app-darkLine bg-app-box flex h-10 w-full flex-row items-center justify-between border px-3 shadow-sm',
						dropdownClosed ? 'rounded' : 'border-b-app-box rounded-t'
					)}
				>
					<Text style={tw`text-ink text-sm font-semibold`}>{currentLibrary?.config.name}</Text>
					<MotiView
						animate={{
							rotateZ: dropdownClosed ? '0deg' : '90deg'
						}}
						transition={{ type: 'timing' }}
					>
						<CaretRight color={tw.color('text-ink')} size={16} style={tw`ml-2`} />
					</MotiView>
				</View>
			</Pressable>
			<AnimatedHeight hide={dropdownClosed}>
				<View style={tw`border-app-darkLine bg-app-box rounded-b border-x border-b p-2`}>
					{/* Libraries */}
					{libraries?.map((library) => (
						<Pressable key={library.uuid} onPress={() => switchLibrary(library.uuid)}>
							<View
								style={twStyle(
									'mt-1 p-2',
									currentLibrary.uuid === library.uuid && 'bg-accent rounded'
								)}
							>
								<Text
									style={twStyle(
										'text-ink text-sm font-semibold',
										currentLibrary.uuid === library.uuid && 'text-white'
									)}
								>
									{library.config.name}
								</Text>
							</View>
						</Pressable>
					))}
					<Divider style={tw`my-2`} />
					{/* Menu */}
					<Pressable
						onPress={() => navigation.navigate('Settings', { screen: 'LibraryGeneralSettings' })}
					>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Gear size={16} color={tw.color('ink-dull')} style={tw`mr-2`} />
							<Text style={tw`text-ink text-sm font-semibold`}>Library Settings</Text>
						</View>
					</Pressable>
					{/* Create Library */}
					<CreateLibraryDialog>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Plus size={16} weight="bold" color={tw.color('ink-dull')} style={tw`mr-2`} />
							<Text style={tw`text-ink text-sm font-semibold`}>Add Library</Text>
						</View>
					</CreateLibraryDialog>
					<Pressable onPress={() => console.log('TODO: lock')}>
						<View style={tw`flex flex-row items-center px-1.5 py-[8px]`}>
							<Lock size={16} weight="bold" color={tw.color('ink-dull')} style={tw`mr-2`} />
							<Text style={tw`text-ink text-sm font-semibold`}>Lock</Text>
						</View>
					</Pressable>
				</View>
			</AnimatedHeight>
		</View>
	);
};

export default DrawerLibraryManager;
