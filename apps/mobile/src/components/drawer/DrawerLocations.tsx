import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import tw from '~/lib/tailwind';
import FolderIcon from '../icons/FolderIcon';
import CollapsibleView from '../layout/CollapsibleView';
import ImportModal from '../modal/ImportModal';

type DrawerLocationItemProps = {
	folderName: string;
	onPress: () => void;
};

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = (props) => {
	const { folderName, onPress } = props;

	return (
		<Pressable onPress={onPress}>
			<View style={tw.style('flex mb-[4px] flex-row items-center py-2 px-1 rounded')}>
				<FolderIcon size={20} />
				<Text style={tw.style('text-gray-300 font-medium ml-1.5')} numberOfLines={1}>
					{folderName}
				</Text>
			</View>
		</Pressable>
	);
};

type DrawerLocationsProp = {
	stackName: string;
};

const DrawerLocations = ({ stackName }: DrawerLocationsProp) => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const importModalRef = useRef<ModalRef>();

	const { data: locations } = useLibraryQuery(['locations.list'], { keepPreviousData: true });

	return (
		<>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`text-sm font-semibold text-gray-300`}
				containerStyle={tw`mt-6 mb-3 ml-1`}
			>
				<View style={tw`mt-2`}>
					{locations?.map((location) => (
						<DrawerLocationItem
							key={location.id}
							folderName={location.name}
							onPress={() =>
								navigation.navigate(stackName, {
									screen: 'Location',
									params: { id: location.id }
								})
							}
						/>
					))}
				</View>
				{/* Add Location */}
				<Pressable onPress={() => importModalRef.current.present()}>
					<View style={tw`border border-dashed rounded border-app-line border-opacity-80 mt-1`}>
						<Text style={tw`text-xs font-bold text-center text-gray-400 px-2 py-2`}>
							Add Location
						</Text>
					</View>
				</Pressable>
			</CollapsibleView>
			<ImportModal ref={importModalRef} />
		</>
	);
};

export default DrawerLocations;
