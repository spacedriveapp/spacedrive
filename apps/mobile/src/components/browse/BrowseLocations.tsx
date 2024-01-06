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
import Fade from '../layout/Fade';
import ImportModal from '../modal/ImportModal';
import { LocationModal } from '../modal/location/LocationModal';

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
			<View
				style={tw`h-fit w-[100px] flex-col justify-center gap-3 rounded-md border border-sidebar-line/50 bg-sidebar-box p-2`}
			>
				<View style={tw`flex-col justify-between w-full gap-1`}>
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
						style={tw`w-full max-w-[75px] text-xs font-bold text-white`}
						numberOfLines={1}
					>
						{location.name}
					</Text>
				</View>
				<Text style={tw`text-left text-[13px] font-bold text-ink-dull`} numberOfLines={1}>
					{`${byteSize(location.size_in_bytes)}`}
				</Text>
			</View>
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
		<View style={tw`gap-5`}>
			<View style={tw`flex-row items-center justify-between w-full px-7`}>
				<Text style={tw`text-xl font-bold text-white`}>Locations</Text>
				<View style={tw`flex-row gap-3`}>
					<Pressable>
						<View style={tw`items-center justify-center w-8 h-8 rounded-md bg-accent`}>
							<Eye weight="bold" size={18} style={tw`text-white`} />
						</View>
					</Pressable>
					<Pressable onPress={() => modalRef.current?.present()}>
						<View
							style={tw`items-center justify-center w-8 h-8 bg-transparent border border-dashed rounded-md border-ink-faint`}
						>
							<Plus weight="bold" size={18} style={tw`text-ink-faint`} />
						</View>
					</Pressable>
				</View>
			</View>
			<Fade color="mobile-screen" width={30} height="100%">
				<FlatList
					data={locations}
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
