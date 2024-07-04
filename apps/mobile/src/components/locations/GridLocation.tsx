import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { arraysEqual, humanizeSize, Location, useOnlineLocations } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import FolderIcon from '../icons/FolderIcon';
import Card from '../layout/Card';
import { ModalRef } from '../layout/Modal';

interface GridLocationProps {
	location: Location;
	modalRef: React.RefObject<ModalRef>;
}

const GridLocation: React.FC<GridLocationProps> = ({ location, modalRef }: GridLocationProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	return (
		<Card style={'h-auto flex-col items-start justify-center gap-3'}>
			<View style={tw`w-full flex-col justify-between gap-1`}>
				<View style={tw`flex-row items-center justify-between`}>
					<View style={tw`relative`}>
						<FolderIcon size={36} />
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
					style={tw`w-full max-w-[100px] text-xs font-bold text-white`}
					numberOfLines={1}
				>
					{location.name}
				</Text>
				<Text numberOfLines={1} style={tw`text-xs text-ink-dull`}>
					{location.path}
				</Text>
			</View>
			<View style={tw`rounded-md border border-app-box/70 bg-app/70 px-1 py-0.5`}>
				<Text style={tw`text-xs font-bold text-ink-dull`} numberOfLines={1}>
					{`${humanizeSize(location.size_in_bytes)}`}
				</Text>
			</View>
		</Card>
	);
};

export default GridLocation;
