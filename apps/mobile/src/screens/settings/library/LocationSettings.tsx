import { Location, Node, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Repeat, Trash } from 'phosphor-react-native';
import React from 'react';
import { FlatList, Text, View } from 'react-native';
import FolderIcon from '~/components/icons/FolderIcon';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import DeleteLocationDialog from '~/containers/dialog/DeleteLocationDialog';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

function LocationListItem({
	location,
	index
}: {
	location: Location & { node: Node };
	index: number;
}) {
	const { mutate: fullRescan } = useLibraryMutation('locations.fullRescan', {
		onMutate: () => {
			// TODO: Show Toast
		}
	});

	return (
		<Card style={tw.style(index !== 0 && 'mt-2')}>
			<View style={tw`flex flex-row items-center`}>
				<View style={tw`relative`}>
					<FolderIcon size={32} />
					{/* Online/Offline Indicator */}
					<View
						style={tw.style(
							'absolute w-2 h-2 right-0 bottom-0.5 rounded-full',
							location.is_online ? 'bg-green-500' : 'bg-red-500'
						)}
					/>
				</View>
				<View style={tw`flex-1 ml-4`}>
					<Text numberOfLines={1} style={tw`text-sm font-semibold text-ink`}>
						{location.name}
					</Text>
					<View style={tw`self-start bg-app-highlight py-[1px] px-1 rounded mt-0.5`}>
						<Text numberOfLines={1} style={tw`text-xs font-semibold text-ink-dull`}>
							{location.node.name}
						</Text>
					</View>
					<Text numberOfLines={1} style={tw`mt-0.5 text-[10px] font-semibold text-ink-dull`}>
						{location.local_path}
					</Text>
				</View>
				<View style={tw`flex flex-row`}>
					<DeleteLocationDialog locationId={location.id}>
						<Button disabled size="sm" style={tw`opacity-100`}>
							<Trash size={18} color="white" />
						</Button>
					</DeleteLocationDialog>
					{/* Full Re-scan IS too much here */}
					<Button size="sm" style={tw`ml-1`} onPress={() => fullRescan(location.id)}>
						<Repeat size={18} color="white" />
					</Button>
				</View>
			</View>
		</Card>
	);
}

// TODO: Add new location from here (ImportModal)

const LocationSettingsScreen = ({ navigation }: SettingsStackScreenProps<'LocationSettings'>) => {
	const { data: locations } = useLibraryQuery(['locations.list']);

	return (
		<View style={tw`px-3 py-4`}>
			<FlatList
				data={locations}
				keyExtractor={(item) => item.id.toString()}
				renderItem={({ item, index }) => <LocationListItem location={item} index={index} />}
			/>
		</View>
	);
};

export default LocationSettingsScreen;
