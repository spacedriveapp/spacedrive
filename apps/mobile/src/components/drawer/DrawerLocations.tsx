import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { byteSize, useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';

import FolderIcon from '../icons/FolderIcon';
import CollapsibleView from '../layout/CollapsibleView';
import ImportModal from '../modal/ImportModal';

type DrawerLocationItemProps = {
	folderName: string;
	onPress: () => void;
	size: number[] | null;
};

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = ({
	folderName,
	size,
	onPress
}: DrawerLocationItemProps) => {
	return (
		<Pressable onPress={onPress}>
			<View
				style={twStyle(
					'bg-app-darkBox border border-app-inputborder/50 rounded-full w-full h-auto justify-between flex-row items-center rounded p-2'
				)}
			>
				<View style={tw`flex-row items-center gap-1`}>
					<FolderIcon size={24} />
					<Text style={twStyle('font-medium text-xs text-ink')} numberOfLines={1}>
						{folderName}
					</Text>
				</View>
				<View style={tw`py-0.5 px-1 border rounded-md border-app-lightborder bg-app-box`}>
					<Text style={tw`text-[11px] font-medium text-ink-dull`} numberOfLines={1}>
						{`${byteSize(size)}`}
					</Text>
				</View>
			</View>
		</Pressable>
	);
};

const DrawerLocations = () => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const modalRef = useRef<ModalRef>(null);

	const result = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	useNodes(result.data?.nodes);
	const locations = useCache(result.data?.items);

	return (
		<>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`text-sm font-semibold text-ink`}
				containerStyle={tw`mt-6 mb-3 ml-1`}
			>
				<View style={tw`flex-col justify-between gap-1 mt-2`}>
					{locations?.slice(0, 4).map((location) => (
						<DrawerLocationItem
							key={location.id}
							size={location.size_in_bytes}
							folderName={location.name ?? ''}
							onPress={() =>
								navigation.navigate('BrowseStack', {
									screen: 'Location',
									params: { id: location.id },
									initial: false
								})
							}
						/>
					))}
				</View>
				{/* Add Location */}
				<Pressable onPress={() => modalRef.current?.present()}>
					<View style={tw`mt-2 border border-dashed rounded border-app-line/80`}>
						<Text style={tw`p-2 text-xs font-bold text-center text-ink-dull`}>
							Add Location
						</Text>
					</View>
				</Pressable>
			</CollapsibleView>
			<ImportModal ref={modalRef} />
		</>
	);
};

export default DrawerLocations;
