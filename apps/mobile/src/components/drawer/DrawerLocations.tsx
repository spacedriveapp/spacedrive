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
import { Button } from '../primitive/Button';

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
					'h-auto w-full flex-row items-center justify-between rounded border border-app-inputborder/50 bg-app-darkBox p-2'
				)}
			>
				<View style={tw`flex-row items-center gap-1`}>
					<FolderIcon size={24} />
					<Text style={twStyle('text-xs font-medium text-ink')} numberOfLines={1}>
						{folderName}
					</Text>
				</View>
				<View style={tw`rounded-md border border-app-lightborder bg-app-box px-1 py-0.5`}>
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
				containerStyle={tw`mb-3 ml-1 mt-6`}
			>
				<View style={tw`mt-2 flex-col justify-between gap-1`}>
					{locations?.slice(0, 3).map((location) => (
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
				<View style={tw`mt-2 flex-row gap-2`}>
					{/* Add Location */}
					<Button
						style={tw`flex-1 py-0`}
						onPress={() => modalRef.current?.present()}
						variant="dashed"
					>
						<Text style={tw`p-2 text-center text-xs font-medium text-ink-dull`}>
							+ Location
						</Text>
					</Button>
					{/* See all locations */}
					{locations?.length > 3 && (
						<Button
							onPress={() => {
								navigation.navigate('BrowseStack', {
									screen: 'Locations',
									initial: false
								});
							}}
							style={tw`flex-1 py-0`}
							variant="gray"
						>
							<Text style={tw`p-2 text-center text-xs font-medium text-ink`}>
								View all
							</Text>
						</Button>
					)}
				</View>
			</CollapsibleView>
			<ImportModal ref={modalRef} />
		</>
	);
};

export default DrawerLocations;
