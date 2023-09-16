import { GoogleDrive, iCloud, Mega } from '@sd/assets/images';
import { DeviceMobile, Icon, Laptop, User } from 'phosphor-react-native';
import { Alert, Image, ImageSourcePropType, Pressable, ScrollView, Text, View } from 'react-native';
import { Polygon, Svg } from 'react-native-svg';

import { InfoPill } from '~/components/primitive/InfoPill';
import { tw, twStyle } from '~/lib/tailwind';
import { SpacedropStackScreenProps } from '~/navigation/tabs/SpacedropStack';

const testData = [
	{
		name: "Jamie's MacBook Pro",
		receivingNodeOsType: 'macOS',
		connectionType: 'lan',
		icon: Laptop
	},
	{
		name: "Jamie's MacBook Pro",
		receivingNodeOsType: 'iOS',
		connectionType: 'lan',
		icon: DeviceMobile
	},
	{
		name: 'Brendan Alan',
		image: 'https://github.com/brendonovich.png',
		connectionType: 'p2p'
	},
	{
		name: 'Oscar Beaumont',
		image: 'https://github.com/oscartbeaumont.png',
		connectionType: 'usb'
	},
	{
		name: 'maxichrome',
		image: 'https://github.com/maxichrome.png',
		connectionType: 'p2p'
	},
	{
		name: 'Utku',
		image: 'https://github.com/utkubakir.png',
		connectionType: 'p2p'
	},
	{ name: "Jamie's Google Drive", brandIcon: 'google-drive', connectionType: 'cloud' },
	{ name: 'iCloud', brandIcon: 'icloud', connectionType: 'cloud' },
	{ name: 'Mega', brandIcon: 'mega', connectionType: 'cloud' }
] as DropItemProps[];

const Hexagon = () => {
	const width = 180;
	const height = width * 1.1547;

	return (
		<Svg width={width} height={height} viewBox="0 0 100 100">
			<Polygon
				points="0,25 0,75 50,100 100,75 100,25 50,0"
				fill={tw.color('bg-app-box/30')}
			/>
		</Svg>
	);
};

type OperatingSystem = 'browser' | 'linux' | 'macOS' | 'windows' | 'iOS' | 'android';

type DropItemProps = {
	name: string;
	connectionType: 'lan' | 'bluetooth' | 'usb' | 'p2p' | 'cloud';
	receivingNodeOsType: OperatingSystem;
} & ({ image: string } | { icon: Icon } | { brandIcon: string });

function DropItem(props: DropItemProps) {
	let icon;
	if ('image' in props) {
		icon = <Image style={tw`h-12 w-12 rounded-full`} source={{ uri: props.image }} />;
	} else if ('brandIcon' in props) {
		let brandIconSrc: ImageSourcePropType | undefined;
		switch (props.brandIcon) {
			case 'google-drive':
				brandIconSrc = GoogleDrive;
				break;
			case 'icloud':
				brandIconSrc = iCloud;
				break;
			case 'mega':
				brandIconSrc = Mega;
				break;
		}
		if (!brandIconSrc) throw new Error('Invalid brand icon url: ' + props.brandIcon);
		icon = (
			<View style={tw`flex items-center justify-center p-3`}>
				{/* // Needs width and height */}
				<Image source={brandIconSrc} style={tw`h-8 w-8 rounded-full`} />
			</View>
		);
	} else {
		// Use the custom icon or default to User icon.
		const Icon = props.icon || User;
		icon = <Icon size={30} color="white" style={twStyle(!props.name && 'opacity-20')} />;
	}
	return (
		<View style={tw`relative`}>
			<Hexagon />
			<View style={tw`absolute h-full w-full items-center justify-center`}>
				<Pressable
					style={tw`w-full items-center justify-center`}
					onPress={() => Alert.alert('TODO')}
				>
					<View
						style={tw`h-12 w-12 items-center justify-center rounded-full bg-app-button`}
					>
						{icon}
					</View>
					{props.name && (
						<Text numberOfLines={1} style={tw`mt-1 text-sm font-medium text-white`}>
							{props.name}
						</Text>
					)}
					<View style={tw`mt-1 flex flex-row gap-x-1`}>
						{props.receivingNodeOsType && <InfoPill text={props.receivingNodeOsType} />}
						{props.connectionType && (
							<InfoPill
								text={props.connectionType}
								containerStyle={twStyle(
									'px-1',
									props.connectionType === 'lan' && 'bg-green-500',
									props.connectionType === 'p2p' && 'bg-blue-500'
								)}
								textStyle={tw`uppercase text-white`}
							/>
						)}
					</View>
				</Pressable>
			</View>
		</View>
	);
}

export default function SpacedropScreen({ navigation }: SpacedropStackScreenProps<'Spacedrop'>) {
	return (
		<View style={tw`flex-1 py-4`}>
			<ScrollView contentContainerStyle={tw`flex flex-row flex-wrap justify-center gap-x-2`}>
				{testData.map((item, i) => (
					<DropItem key={i} {...item} />
				))}
			</ScrollView>
		</View>
	);
}
