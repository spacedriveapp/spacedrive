import { Cloud, Desktop, DeviceMobileCamera, Laptop } from 'phosphor-react-native';
import React from 'react';
import { FlatList, Text, View } from 'react-native';
import { LockClosedIcon } from 'react-native-heroicons/solid';

import tw from '../../lib/tailwind';
import FileItem from '../file/FileItem';

export interface DeviceProps {
	name: string;
	size: string;
	type: 'laptop' | 'desktop' | 'phone' | 'server';
	locations: Array<{ name: string; folder?: boolean; format?: string; icon?: string }>;
}

export interface FilePath {
	id: number;
	is_dir: boolean;
	location_id: number | null;
	materialized_path: string;
	name: string;
	extension: string | null;
	file_id: number | null;
	parent_id: number | null;
	key_id: number | null;
	date_created: string;
	date_modified: string;
	date_indexed: string;
	file: any;
	location: Location | null | null;
	key: any;
}

const placeholderFileItems: FilePath[] = [
	{
		is_dir: true,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		file_id: 1,
		id: 1,
		key: null,
		location_id: 1,
		materialized_path: '',
		name: 'Minecraft',
		parent_id: 0,
		key_id: null,
		location: null,
		file: {
			id: 1,
			key_id: 1,
			albums: [],
			comments: [],
			key: {
				algorithm: null,
				checksum: '',
				date_created: null,
				file_paths: [],
				files: [],
				id: 1,
				name: 'Hello world'
			},
			labels: [],
			media_data: null,
			spaces: [],
			tags: [],
			cas_id: '',
			ipfs_id: '',
			has_thumbnail: false,
			favorite: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			integrity_checksum: '',
			kind: 1,
			note: '',
			paths: [],
			size_in_bytes: '555',
			date_created: '',
			date_indexed: '',
			date_modified: ''
		}
	},
	{
		is_dir: true,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: '',
		file_id: 2,
		id: 2,
		key: null,
		location_id: 2,
		materialized_path: '',
		name: 'Documents',
		parent_id: 0,
		key_id: null,
		location: null,
		file: {
			id: 2,
			key_id: 2,
			albums: [],
			comments: [],
			key: {
				algorithm: null,
				checksum: '',
				date_created: null,
				file_paths: [],
				files: [],
				id: 1,
				name: 'Hello world'
			},
			labels: [],
			media_data: null,
			spaces: [],
			tags: [],
			cas_id: '',
			ipfs_id: '',
			has_thumbnail: false,
			favorite: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			integrity_checksum: '',
			kind: 1,
			note: '',
			paths: [],
			size_in_bytes: '555',
			date_created: '',
			date_indexed: '',
			date_modified: ''
		}
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'tsx',
		file_id: 3,
		id: 3,
		key: null,
		location_id: 3,
		materialized_path: '',
		name: 'App.tsx',
		parent_id: 0,
		key_id: null,
		location: null,
		file: {
			id: 3,
			key_id: 3,
			albums: [],
			comments: [],
			key: {
				algorithm: null,
				checksum: '',
				date_created: null,
				file_paths: [],
				files: [],
				id: 1,
				name: 'Hello world'
			},
			labels: [],
			media_data: null,
			spaces: [],
			tags: [],
			cas_id: '',
			ipfs_id: '',
			has_thumbnail: false,
			favorite: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			integrity_checksum: '',
			kind: 1,
			note: '',
			paths: [],
			size_in_bytes: '555',
			date_created: '',
			date_indexed: '',
			date_modified: ''
		}
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'vite',
		file_id: 4,
		id: 4,
		key: null,
		location_id: 4,
		materialized_path: '',
		name: 'vite.config.js',
		parent_id: 0,
		key_id: null,
		location: null,
		file: {
			id: 4,
			key_id: 4,
			albums: [],
			comments: [],
			key: {
				algorithm: null,
				checksum: '',
				date_created: null,
				file_paths: [],
				files: [],
				id: 1,
				name: 'Hello world'
			},
			labels: [],
			media_data: null,
			spaces: [],
			tags: [],
			cas_id: '',
			ipfs_id: '',
			has_thumbnail: false,
			favorite: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			integrity_checksum: '',
			kind: 1,
			note: '',
			paths: [],
			size_in_bytes: '555',
			date_created: '',
			date_indexed: '',
			date_modified: ''
		}
	},
	{
		is_dir: false,
		date_created: '2020-01-01T00:00:00.000Z',
		date_indexed: '2020-01-01T00:00:00.000Z',
		date_modified: '2020-01-01T00:00:00.000Z',
		extension: 'docker',
		file_id: 5,
		id: 5,
		key: null,
		location_id: 5,
		materialized_path: '',
		name: 'Dockerfile',
		parent_id: 0,
		key_id: null,
		location: null,
		file: {
			id: 5,
			key_id: 5,
			albums: [],
			comments: [],
			key: {
				algorithm: null,
				checksum: '',
				date_created: null,
				file_paths: [],
				files: [],
				id: 1,
				name: 'Hello world'
			},
			labels: [],
			media_data: null,
			spaces: [],
			tags: [],
			cas_id: '',
			ipfs_id: '',
			has_thumbnail: false,
			favorite: false,
			has_thumbstrip: false,
			has_video_preview: false,
			hidden: false,
			important: false,
			integrity_checksum: '',
			kind: 1,
			note: '',
			paths: [],
			size_in_bytes: '555',
			date_created: '',
			date_indexed: '',
			date_modified: ''
		}
	}
];

const Device = ({ name, locations, size, type }: DeviceProps) => {
	return (
		<View style={tw`bg-gray-600 border rounded-md border-gray-550 my-2`}>
			<View style={tw`flex flex-row items-center px-4 pt-3 pb-2`}>
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
				<Text style={tw`font-semibold text-sm ml-2 text-gray-400`}>{size}</Text>
			</View>
			{/* Locations/Files TODO: Maybe use FlashList? */}
			<FlatList
				data={placeholderFileItems}
				renderItem={({ item }) => <FileItem file={item} />}
				keyExtractor={(item) => item.id.toString()}
				horizontal
				contentContainerStyle={tw`mt-4 ml-2`}
				showsHorizontalScrollIndicator={false}
			/>
		</View>
	);
};

export default Device;
