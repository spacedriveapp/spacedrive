import { Cloud, Desktop, DeviceMobileCamera, Laptop } from 'phosphor-react-native';
import React from 'react';
import { FlatList, Text, View } from 'react-native';
import { LockClosedIcon } from 'react-native-heroicons/solid';
import tw from '~/lib/tailwind';
import { ExplorerItem } from '~/types/bindings';

import FileItem from '../explorer/FileItem';

const placeholderFileItems: ExplorerItem[] = [
	{
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		id: 2,
		name: 'Documents',
		key_id: null,
		type: 'Path',
		is_dir: true,
		location_id: 1,
		materialized_path: '/Documents',
		object_id: 5,
		parent_id: 1,
		object: {
			extension: 'tsx',
			cas_id: '3',
			id: 3,
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
			integrity_checksum: '',
			ipfs_id: '',
			kind: 5,
			note: '',
			size_in_bytes: '0'
		}
	},
	{
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		id: 1,
		name: 'Minecraft',
		key_id: null,
		type: 'Object',
		cas_id: '555',
		favorite: false,
		file_paths: [],
		has_thumbnail: false,
		has_thumbstrip: false,
		has_video_preview: false,
		hidden: false,
		important: false,
		integrity_checksum: '',
		ipfs_id: '',
		kind: 5,
		note: '',
		size_in_bytes: '0'
	},
	{
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		id: 5,
		name: 'Minecraft',
		key_id: null,
		type: 'Object',
		cas_id: '555',
		favorite: false,
		file_paths: [],
		has_thumbnail: false,
		has_thumbstrip: false,
		has_video_preview: false,
		hidden: false,
		important: false,
		integrity_checksum: '',
		ipfs_id: '',
		kind: 5,
		note: '',
		size_in_bytes: '0'
	}
];

export interface DeviceProps {
	name: string;
	size: string;
	type: 'laptop' | 'desktop' | 'phone' | 'server';
	locations: { name: string; folder?: boolean; format?: string; icon?: string }[];
	runningJob?: { amount: number; task: string };
}

const Device = ({ name, locations, size, type }: DeviceProps) => {
	return (
		<View style={tw`my-2 bg-gray-600 border rounded-md border-gray-550`}>
			<View style={tw`flex flex-row items-center px-3.5 pt-3 pb-2`}>
				<View style={tw`flex flex-row items-center`}>
					{type === 'phone' && (
						<DeviceMobileCamera color="white" weight="fill" size={18} style={tw`mr-2`} />
					)}
					{type === 'laptop' && <Laptop color="white" weight="fill" size={18} style={tw`mr-2`} />}
					{type === 'desktop' && <Desktop color="white" weight="fill" size={18} style={tw`mr-2`} />}
					{type === 'server' && <Cloud color="white" weight="fill" size={18} style={tw`mr-2`} />}
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
				renderItem={({ item }) => <FileItem data={item} />}
				keyExtractor={(item) => item.id.toString()}
				horizontal
				contentContainerStyle={tw`mt-3 mb-5`}
				showsHorizontalScrollIndicator={false}
			/>
		</View>
	);
};

export default Device;
