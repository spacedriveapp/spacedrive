import { Cloud, Desktop, DeviceMobileCamera, Laptop } from 'phosphor-react-native';
import React from 'react';
import { Text, View } from 'react-native';
import { LockClosedIcon } from 'react-native-heroicons/solid';

import tw from '../../lib/tailwind';

export interface DeviceProps {
	name: string;
	size: string;
	type: 'laptop' | 'desktop' | 'phone' | 'server';
	locations: { name: string; folder?: boolean; format?: string; icon?: string }[];
}

const Device = ({ name, locations, size, type }: DeviceProps) => {
	return (
		<View style={tw`bg-gray-600 border rounded-md border-gray-550 mt-4`}>
			<View style={tw`flex flex-row items-center justify-between px-4 pt-3 pb-2`}>
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
			<View style={tw`mt-4 p-4`} />
		</View>
	);
};

export default Device;
