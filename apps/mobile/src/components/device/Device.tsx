import { Cloud, Desktop, DeviceMobileCamera, Laptop } from 'phosphor-react-native';
import { FlatList, Text, View } from 'react-native';
import { LockClosedIcon } from 'react-native-heroicons/solid';
import tw from '~/lib/tailwind';
import { FilePath } from '~/types/bindings';

import FileItem from '../file/FileItem';

const placeholderFileItems: FilePath[] = [
	{
		is_dir: true,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		object_id: 1,
		id: 1,
		location_id: 1,
		materialized_path: '',
		name: 'Minecraft',
		parent_id: 0,
		key_id: null
	},
	{
		is_dir: true,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		object_id: 2,
		id: 2,
		location_id: 2,
		materialized_path: '',
		name: 'Documents',
		parent_id: 0,
		key_id: null
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'tsx',
		object_id: 3,
		id: 3,
		location_id: 3,
		materialized_path: '',
		name: 'App.tsx',
		parent_id: 0,
		key_id: null
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'vite',
		object_id: 4,
		id: 4,
		location_id: 4,
		materialized_path: '',
		name: 'vite.config.js',
		parent_id: 0,
		key_id: null
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'docker',
		object_id: 5,
		id: 5,
		location_id: 5,
		materialized_path: '',
		name: 'Dockerfile',
		parent_id: 0,
		key_id: null
	}
];

export interface DeviceProps {
	name: string;
	size: string;
	type: keyof typeof DeviceIcon;
	locations: { name: string; folder?: boolean; format?: string; icon?: string }[];
	runningJob?: { amount: number; task: string };
}

const DeviceIcon = {
	phone: <DeviceMobileCamera color="white" weight="fill" size={18} style={tw`mr-2`} />,
	laptop: <Laptop color="white" weight="fill" size={18} style={tw`mr-2`} />,
	desktop: <Desktop color="white" weight="fill" size={18} style={tw`mr-2`} />,
	server: <Cloud color="white" weight="fill" size={18} style={tw`mr-2`} />
};

const Device = ({ name, locations, size, type }: DeviceProps) => {
	return (
		<View style={tw`my-2 bg-gray-600 border rounded-md border-gray-550`}>
			<View style={tw`flex flex-row items-center px-3.5 pt-3 pb-2`}>
				<View style={tw`flex flex-row items-center`}>
					{DeviceIcon[type]}
					<Text style={tw`text-base font-semibold text-white`}>{name || 'Unnamed Device'}</Text>
					{/* P2P Lock */}
					<View style={tw`flex flex-row rounded items-center ml-2 bg-gray-500 py-[1px] px-[4px]`}>
						<LockClosedIcon size={12} color={tw.color('gray-400')} />
						<Text style={tw`text-gray-400 font-semibold ml-0.5 text-xs`}>P2P</Text>
					</View>
				</View>
				{/* Size */}
				<Text style={tw`ml-2 text-sm font-semibold text-gray-400`}>{size}</Text>
			</View>
			{/* Locations/Files TODO: Maybe use FlashList? */}
			<FlatList
				data={placeholderFileItems}
				renderItem={({ item }) => <FileItem file={item} />}
				keyExtractor={(item) => item.id.toString()}
				horizontal
				contentContainerStyle={tw`mt-3 mb-5`}
				showsHorizontalScrollIndicator={false}
			/>
		</View>
	);
};

export default Device;
