import { Icon, User } from 'phosphor-react-native';
import { Image, StyleSheet, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { SpacedropStackScreenProps } from '~/navigation/tabs/SpacedropStack';

type OperatingSystem = 'browser' | 'linux' | 'macOS' | 'windows';

type DropItemProps = {
	name: string;
	connectionType: 'lan' | 'bluetooth' | 'usb' | 'p2p' | 'cloud';
	receivingNodeOsType: OperatingSystem;
} & ({ image: string } | { icon: Icon } | { brandIcon: string });

function DropItem(props: DropItemProps) {
	let icon;
	if ('image' in props) {
		// Needs width and height
		icon = <Image style={tw`rounded-full`} source={{ uri: props.image }} />;
	} else if ('brandIcon' in props) {
		let brandIconSrc;
		switch (props.brandIcon) {
			case 'google-drive':
				brandIconSrc = '@sd/assets/images/GoogleDrive.png';
				break;
			case 'icloud':
				brandIconSrc = '@sd/assets/images/Mega.png';
				break;
			case 'mega':
				brandIconSrc = '@sd/assets/images/iCloud.png';
				break;
		}
		if (!brandIconSrc) throw new Error('Invalid brand icon url: ' + props.brandIcon);
		icon = (
			<View style={tw`flex h-full items-center justify-center p-3`}>
				{/* // Needs width and height */}
				<Image source={require('@sd/assets/images/Mega.png')} style={tw`rounded-full`} />
			</View>
		);
	} else {
		// Use the custom icon or default to User icon.
		const Icon = props.icon || User;
		icon = <Icon style={twStyle('m-3 h-8 w-8', !props.name && 'opacity-20')} />;
	}
	return (
		<View>
			<View></View>
		</View>
	);
}

export default function SpacedropScreen({ navigation }: SpacedropStackScreenProps<'Spacedrop'>) {
	return (
		<View style={tw`flex-1`}>
			<View style={tw`flex flex-row flex-wrap`}>
				{Array.from({ length: 10 }).map((_, i) => (
					<View key={i} style={styles.hexagon}>
						<View style={styles.hexagonInner} />
						<View style={styles.hexagonBefore} />
						<View style={styles.hexagonAfter} />
					</View>
				))}
			</View>
		</View>
	);
}

const HEXAGON_WIDTH = 160;
const HEXAGON_COLOR = tw.color('bg-app-box/50');

const styles = StyleSheet.create({
	hexagon: {
		width: HEXAGON_WIDTH,
		height: HEXAGON_WIDTH * 0.55,
		margin: 4
	},
	hexagonInner: {
		width: HEXAGON_WIDTH,
		height: HEXAGON_WIDTH * 0.55,
		backgroundColor: HEXAGON_COLOR
	},
	hexagonAfter: {
		position: 'absolute',
		bottom: -(HEXAGON_WIDTH / 4),
		left: 0,
		width: 0,
		height: 0,
		borderStyle: 'solid',
		borderLeftWidth: HEXAGON_WIDTH / 2,
		borderLeftColor: 'transparent',
		borderRightWidth: HEXAGON_WIDTH / 2,
		borderRightColor: 'transparent',
		borderTopWidth: HEXAGON_WIDTH / 4,
		borderTopColor: HEXAGON_COLOR
	},
	hexagonBefore: {
		position: 'absolute',
		top: -(HEXAGON_WIDTH / 4),
		left: 0,
		width: 0,
		height: 0,
		borderStyle: 'solid',
		borderLeftWidth: HEXAGON_WIDTH / 2,
		borderLeftColor: 'transparent',
		borderRightWidth: HEXAGON_WIDTH / 2,
		borderRightColor: 'transparent',
		borderBottomWidth: HEXAGON_WIDTH / 4,
		borderBottomColor: HEXAGON_COLOR
	}
});
