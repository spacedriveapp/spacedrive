import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import {
	arraysEqual,
	humanizeSize,
	Location,
	useLibraryQuery,
	useOnlineLocations
} from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';

import FolderIcon from '../icons/FolderIcon';
import CollapsibleView from '../layout/CollapsibleView';
import ImportModal from '../modal/ImportModal';
import { Button } from '../primitive/Button';

type DrawerLocationItemProps = {
	onPress: () => void;
	location: Location;
};

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = ({
	location,
	onPress
}: DrawerLocationItemProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	return (
		<Pressable onPress={onPress}>
			<View
				style={twStyle(
					'h-auto w-full flex-row items-center justify-between rounded-md border border-app-inputborder/50 bg-app-darkBox p-2'
				)}
			>
				<View style={tw`flex-row items-center gap-1`}>
					<View style={tw`relative`}>
						<FolderIcon size={20} />
						<View
							style={twStyle(
								'z-5 absolute bottom-1 right-px h-1.5 w-1.5 rounded-full',
								online ? 'bg-green-500' : 'bg-red-500'
							)}
						/>
					</View>
					<Text
						style={twStyle('max-w-[150px] text-xs font-medium text-ink')}
						numberOfLines={1}
					>
						{location.name ?? ''}
					</Text>
				</View>
				<View style={tw`rounded-md border border-app-box/70 bg-app/70 px-1 py-0.5`}>
					<Text style={tw`text-[11px] font-bold text-ink-dull`} numberOfLines={1}>
						{`${humanizeSize(location.size_in_bytes)}`}
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
	const locations = result.data || [];

	return (
		<>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`text-sm font-semibold text-ink`}
				containerStyle={tw`mb-3 mt-6`}
			>
				<View style={tw`mt-2 flex-col justify-between gap-1`}>
					{locations?.slice(0, 3).map((location) => (
						<DrawerLocationItem
							key={location.id}
							location={location}
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
				<View style={tw`mt-2 flex-row flex-wrap gap-1`}>
					{/* Add Location */}
					<Button
						style={twStyle(`py-0`, locations?.length > 3 ? 'w-[49%]' : 'w-full')}
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
							style={tw`w-[49%] py-0`}
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
