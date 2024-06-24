import { useDrawerStatus } from '@react-navigation/drawer';
import { useNavigation } from '@react-navigation/native';
import { useClientContext } from '@sd/client';
import { MotiView } from 'moti';
import { CaretRight, CloudArrowDown, Gear, Lock, Plus } from 'phosphor-react-native';
import { useEffect, useRef, useState } from 'react';
import { Alert, Pressable, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { currentLibraryStore } from '~/utils/nav';

import { AnimatedHeight } from '../animation/layout';
import { ModalRef } from '../layout/Modal';
import CreateLibraryModal from '../modal/CreateLibraryModal';
import ImportModalLibrary from '../modal/ImportLibraryModal';
import { Divider } from '../primitive/Divider';

const DrawerLibraryManager = () => {
	const [dropdownClosed, setDropdownClosed] = useState(true);

	// Closes the dropdown when the drawer is closed
	const isDrawerOpen = useDrawerStatus() === 'open';
	useEffect(() => {
		if (!isDrawerOpen) setDropdownClosed(true);
	}, [isDrawerOpen]);

	const { library: currentLibrary, libraries } = useClientContext();
	const navigation = useNavigation();

	const modalRef = useRef<ModalRef>(null);
	const modalRef_import = useRef<ModalRef>(null);

	return (
		<View>
			<Pressable onPress={() => setDropdownClosed((v) => !v)}>
				<View
					style={twStyle(
						'flex h-10 w-full flex-row items-center justify-between border bg-app-input px-3 shadow-sm',
						dropdownClosed
							? 'rounded-md border-app-inputborder'
							: 'rounded-t-md border-b-0 border-app-inputborder'
					)}
				>
					<Text style={tw`text-sm font-semibold text-ink`}>
						{currentLibrary?.config.name}
					</Text>
					<MotiView
						animate={{ rotateZ: dropdownClosed ? '0deg' : '90deg' }}
						transition={{ type: 'timing', duration: 100 }}
					>
						<CaretRight color="white" size={18} weight="bold" />
					</MotiView>
				</View>
			</Pressable>
			<AnimatedHeight hide={dropdownClosed}>
				<View
					style={tw`w-full rounded-b-md border border-app-inputborder bg-app-input p-2`}
				>
					{/* Libraries */}
					{libraries.data?.map((library) => {
						return (
							<Pressable
								key={library.uuid}
								onPress={() => (currentLibraryStore.id = library.uuid)}
							>
								<View
									style={twStyle(
										'mt-1 p-2',
										currentLibrary?.uuid === library.uuid && 'rounded bg-accent'
									)}
								>
									<Text
										style={twStyle(
											'text-sm font-semibold text-ink',
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
					<Pressable
						style={tw`flex flex-row items-center px-1.5 py-[8px]`}
						onPress={() => modalRef.current?.present()}
					>
						<Plus size={18} weight="bold" color="white" style={tw`mr-2`} />
						<Text style={tw`text-sm font-semibold text-white`}>New Library</Text>
					</Pressable>
					<CreateLibraryModal ref={modalRef} />
					<Pressable
						style={tw`flex flex-row items-center px-1.5 py-[8px]`}
						onPress={() => modalRef_import.current?.present()}
					>
						<CloudArrowDown size={18} weight="bold" color="white" style={tw`mr-2`} />
						<Text style={tw`text-sm font-semibold text-white`}>Import Library</Text>
					</Pressable>
					<ImportModalLibrary ref={modalRef_import} />
					{/* Manage Library */}
					<Pressable
						onPress={() => {
							navigation.navigate('Root', {
								screen: 'Home',
								params: {
									screen: 'SettingsStack',
									params: { screen: 'LibraryGeneralSettings' }
								}
							});
						}}
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
