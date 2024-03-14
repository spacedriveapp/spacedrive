import { useNavigation } from '@react-navigation/native';
import { DotsThreeOutlineVertical, Eye, Plus } from 'phosphor-react-native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { FlatList } from 'react-native-gesture-handler';
import {
	arraysEqual,
	byteSize,
	Location,
	useCache,
	useLibraryQuery,
	useNodes,
	useOnlineLocations
} from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

import FolderIcon from '../icons/FolderIcon';
import { Icon } from '../icons/Icon';
import Fade from '../layout/Fade';
import ImportModal from '../modal/ImportModal';
import { LocationModal } from '../modal/location/LocationModal';
import Card from '../layout/Card';

interface BrowseLocationItemProps {
	location: Location;
	onPress: () => void;
	editLocation: () => void;
}

const BrowseLocationItem: React.FC<BrowseLocationItemProps> = ({
	location,
	editLocation,
	onPress
}: BrowseLocationItemProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	const modalRef = useRef<ModalRef>(null);
	return (
		<Pressable onPress={onPress}>
			<Card style={"h-auto w-[110px] flex-col justify-center gap-3"}>
				<View style={tw`w-full flex-col justify-between gap-1`}>
					<View style={tw`flex-row items-center justify-between`}>
						<View style={tw`relative`}>
							<FolderIcon size={42} />
							<View
								style={twStyle(
									'z-5 absolute bottom-[6px] right-[2px] h-2 w-2 rounded-full',
									online ? 'bg-green-500' : 'bg-red-500'
								)}
							/>
						</View>
						<Pressable onPress={() => modalRef.current?.present()}>
							<DotsThreeOutlineVertical
								weight="fill"
								size={20}
								color={tw.color('ink-faint')}
							/>
						</Pressable>
					</View>
					<Text
						style={tw`w-full max-w-[100px] text-xs font-bold text-white`}
						numberOfLines={1}
					>
						{location.name}
					</Text>
				</View>
				<Text style={tw`text-left text-[13px] font-bold text-ink-dull`} numberOfLines={1}>
					{`${byteSize(location.size_in_bytes)}`}
				</Text>
			</Card>
			<LocationModal
				editLocation={() => {
					editLocation();
					modalRef.current?.close();
				}}
				locationId={location.id}
				ref={modalRef}
			/>
		</Pressable>
	);
};

const BrowseLocations = () => {
	const navigation = useNavigation<
		BrowseStackScreenProps<'Browse'>['navigation'] &
			SettingsStackScreenProps<'Settings'>['navigation']
	>();

	const modalRef = useRef<ModalRef>(null);

	const result = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	useNodes(result.data?.nodes);
	const locations = useCache(result.data?.items);

	return (
		<View style={tw`gap-3`}>
			<View style={tw`w-full flex-row items-center justify-between px-6`}>
				<Text style={tw`text-lg font-bold text-white`}>Locations</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable
						onPress={() => {
							navigation.navigate('Locations');
						}}
					>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md bg-accent`}
						>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable onPress={() => modalRef.current?.present()}>
						<View
							style={tw`h-8 w-8 items-center justify-center rounded-md border border-dashed border-mobile-iconborder bg-transparent`}
						>
							<Plus weight="bold" size={18} style={tw`text-ink`} />
						</View>
					</Pressable>
				</View>
			</View>
			<Fade color="black" width={30} height="100%">
				<FlatList
					data={locations}
					ListEmptyComponent={() => (
						<View
							style={tw`relative h-auto w-[87.5vw] flex-col items-center justify-center overflow-hidden rounded-md border border-dashed border-mobile-lightborder p-4`}
						>
							<Icon name="Folder" size={38} />
							<Text style={tw`mt-2 text-center font-medium text-ink-dull`}>
								You have no locations
							</Text>
						</View>
					)}
					contentContainerStyle={tw`px-7`}
					showsHorizontalScrollIndicator={false}
					ItemSeparatorComponent={() => <View style={tw`w-2`} />}
					renderItem={({ item }) => (
						<BrowseLocationItem
							location={item}
							editLocation={() =>
								navigation.navigate('SettingsStack', {
									screen: 'EditLocationSettings',
									params: { id: item.id }
								})
							}
							onPress={() => navigation.navigate('Location', { id: item.id })}
						/>
					)}
					keyExtractor={(location) => location.id.toString()}
					horizontal
				/>
			</Fade>
			<ImportModal ref={modalRef} />
		</View>
	);
};

export default BrowseLocations;
