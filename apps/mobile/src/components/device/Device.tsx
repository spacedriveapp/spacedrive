import { Cloud, Desktop, DeviceMobileCamera, Laptop, Lock } from 'phosphor-react-native';
import { FlatList, Text, View } from 'react-native';
import { ExplorerItem } from '@sd/client';
import tw from '~/lib/tailwind';
import FileItem from '../explorer/FileItem';

const placeholderFileItems: ExplorerItem[] = [
	{
		type: 'Path',
		item: {
			date_created: '2020-01-01T00:00:00.000Z',
			date_indexed: '2020-01-01T00:00:00.000Z',
			date_modified: '2020-01-01T00:00:00.000Z',
			extension: '',
			cas_id: '3',
			integrity_checksum: '',
			id: 2,
			name: 'Documents',
			key_id: null,
			is_dir: true,
			location_id: 1,
			materialized_path: '/Documents',
			object_id: 5,
			parent_id: 1,
			object: {
				extension: 'tsx',
				id: 3,
				pub_id: [3],
				name: 'App.tsx',
				key_id: null,
				date_created: '2020-01-01T00:00:00.000Z',
				date_indexed: '2020-01-01T00:00:00.000Z',
				date_modified: '2020-01-01T00:00:00.000Z',
				favorite: false,
				has_thumbnail: false,
				has_thumbstrip: false,
				has_video_preview: false,
				hidden: false,
				important: false,
				ipfs_id: '',
				kind: 2,
				note: '',
				size_in_bytes: '0'
			}
		},
		has_thumbnail: false
	},
	{
		type: 'Object',
		has_thumbnail: false,
		item: {
			date_created: '2020-01-01T00:00:00.000Z',
			date_indexed: '2020-01-01T00:00:00.000Z',
			date_modified: '2020-01-01T00:00:00.000Z',
			extension: '',
			id: 1,
			pub_id: [1],
			name: 'Minecraft',
			key_id: null,
			favorite: false,
			file_paths: [],
			has_thumbnail: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			ipfs_id: '',
			kind: 4,
			note: '',
			size_in_bytes: '0'
		}
	},
	{
		type: 'Object',
		has_thumbnail: false,
		item: {
			date_created: '2020-01-01T00:00:00.000Z',
			date_indexed: '2020-01-01T00:00:00.000Z',
			date_modified: '2020-01-01T00:00:00.000Z',
			extension: '',
			id: 5,
			pub_id: [5],
			name: 'Minecraft',
			key_id: null,
			favorite: false,
			file_paths: [],
			has_thumbnail: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			ipfs_id: '',
			kind: 5,
			note: '',
			size_in_bytes: '0'
		}
	}
];

type DeviceProps = {
	name: string;
	size: string;
	type: keyof typeof DeviceIcon;
	locations: { name: string; folder?: boolean; format?: string; icon?: string }[];
	runningJob?: { amount: number; task: string };
};

const DeviceIcon = {
	phone: <DeviceMobileCamera color="white" weight="fill" size={18} style={tw`mr-2`} />,
	laptop: <Laptop color="white" weight="fill" size={18} style={tw`mr-2`} />,
	desktop: <Desktop color="white" weight="fill" size={18} style={tw`mr-2`} />,
	server: <Cloud color="white" weight="fill" size={18} style={tw`mr-2`} />
};

const Device = ({ name, size, type }: DeviceProps) => {
	return (
		<View style={tw`bg-app-overlay border-app-line my-2 rounded-md border`}>
			<View style={tw`flex flex-row items-center px-3.5 pt-3 pb-2`}>
				<View style={tw`flex flex-row items-center`}>
					{DeviceIcon[type]}
					<Text style={tw`text-ink text-base font-semibold`}>{name || 'Unnamed Device'}</Text>
					{/* P2P Lock */}
					<View style={tw`bg-app-box ml-2 flex flex-row items-center rounded py-[1px] px-[4px]`}>
						<Lock weight="bold" size={12} color={tw.color('ink-dull')} />
						<Text style={tw`text-ink-dull ml-0.5 text-xs font-semibold`}>P2P</Text>
					</View>
				</View>
				{/* Size */}
				<Text style={tw`text-ink-dull ml-2 text-sm font-semibold`}>{size}</Text>
			</View>
			<FlatList
				data={placeholderFileItems}
				renderItem={({ item }) => <FileItem data={item} />}
				keyExtractor={(item) => item.item.id.toString()}
				horizontal
				contentContainerStyle={tw`mt-3 mb-5`}
				showsHorizontalScrollIndicator={false}
			/>
		</View>
	);
};

export default Device;
