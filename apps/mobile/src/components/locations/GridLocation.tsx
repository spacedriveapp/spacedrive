import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { arraysEqual, byteSize, Location, useOnlineLocations } from '@sd/client';
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
		<Card style={'h-auto flex-col justify-center gap-3'}>
			<View style={tw`w-full flex-col justify-between gap-1`}>
				<View style={tw`flex-row items-center justify-between`}>
					<View style={tw`relative`}>
						<FolderIcon size={42} />
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
			<Text style={tw`text-left text-[13px] font-bold text-ink-dull`} numberOfLines={1}>
				{`${byteSize(location.size_in_bytes)}`}
			</Text>
		</Card>
	);
};

export default GridLocation;
