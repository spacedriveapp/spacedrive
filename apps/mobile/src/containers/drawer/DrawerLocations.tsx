import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { useLibraryQuery } from '@sd/client';
import tw, { twStyle } from '~/lib/tailwind';
import FolderIcon from '../../components/icons/FolderIcon';
import CollapsibleView from '../../components/layout/CollapsibleView';
import ImportModal from '../modal/ImportModal';

type DrawerLocationItemProps = {
	folderName: string;
	onPress: () => void;
};

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = (props) => {
	const { folderName, onPress } = props;

	return (
		<Pressable onPress={onPress}>
			<View style={twStyle('mb-[4px] flex flex-row items-center rounded py-2 px-1')}>
				<FolderIcon size={18} />
				<Text style={twStyle('ml-2 text-sm font-medium text-gray-300')} numberOfLines={1}>
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

	const importModalRef = useRef<BottomSheetModal>();

	const { data: locations } = useLibraryQuery(['locations.list'], { keepPreviousData: true });

	return (
		<>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`text-sm font-semibold text-gray-300`}
				containerStyle={tw`mt-6 mb-3`}
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
					<View style={tw`border-opacity/80 border-app-line mt-1 rounded border border-dashed`}>
						<Text style={tw`p-2 text-center text-xs font-bold text-gray-400`}>Add Location</Text>
					</View>
				</Pressable>
			</CollapsibleView>
			<ImportModal ref={importModalRef} />
		</>
	);
};

export default DrawerLocations;
