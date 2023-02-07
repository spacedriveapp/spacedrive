import { Cloud, Desktop, DeviceMobileCamera, Laptop, Lock } from 'phosphor-react-native';
import { FlatList, Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import FileItem from '../explorer/FileItem';

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
		<View style={tw`my-2 bg-app-overlay border rounded-md border-app-line`}>
			<View style={tw`flex flex-row items-center px-3.5 pt-3 pb-2`}>
				<View style={tw`flex flex-row items-center`}>
					{DeviceIcon[type]}
					<Text style={tw`text-base font-semibold text-ink`}>{name || 'Unnamed Device'}</Text>
					{/* P2P Lock */}
					<View style={tw`flex flex-row rounded items-center ml-2 bg-app-box py-[1px] px-[4px]`}>
						<Lock weight="bold" size={12} color={tw.color('ink-dull')} />
						<Text style={tw`text-ink-dull font-semibold ml-0.5 text-xs`}>P2P</Text>
					</View>
				</View>
				{/* Size */}
				<Text style={tw`ml-2 text-sm font-semibold text-ink-dull`}>{size}</Text>
			</View>
			<FlatList
				data={[]}
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
