import { DotsThreeOutlineVertical } from 'phosphor-react-native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { arraysEqual, byteSize, Location, useOnlineLocations } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

import FolderIcon from '../icons/FolderIcon';
import Card from '../layout/Card';
import { ModalRef } from '../layout/Modal';
import { LocationModal } from '../modal/location/LocationModal';
import RightActions from './RightActions';

interface LocationItemProps {
	location: Location;
	onPress: () => void;
	editLocation: () => void;
	navigation: SettingsStackScreenProps<'LocationSettings'>['navigation'];
}

const LocationItem = ({ location, editLocation, onPress, navigation }: LocationItemProps) => {
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));
	const modalRef = useRef<ModalRef>(null);

	return (
		<Pressable onPress={onPress}>
			<Swipeable
				containerStyle={tw`rounded-md border border-mobile-cardborder bg-mobile-card`}
				enableTrackpadTwoFingerGesture
				renderRightActions={(progress, _, swipeable) => (
					<>
						<RightActions
							progress={progress}
							swipeable={swipeable}
							location={location}
							navigation={navigation}
						/>
					</>
				)}
			>
				<Card style={tw`h-auto flex-row justify-between gap-3 border-0 p-3`}>
					<View style={tw`w-[50%] flex-row items-center gap-2`}>
						<View style={tw`relative`}>
							<FolderIcon size={38} />
							<View
								style={twStyle(
									'z-5 absolute bottom-[6px] right-[2px] h-2 w-2 rounded-full',
									online ? 'bg-green-500' : 'bg-red-500'
								)}
							/>
						</View>
						<View>
							<Text
								style={tw`w-auto max-w-[160px] text-sm font-bold text-white`}
								numberOfLines={1}
							>
								{location.name}
							</Text>
							<Text numberOfLines={1} style={tw`text-xs text-ink-dull`}>
								{location.path}
							</Text>
						</View>
					</View>
					<View style={tw`flex-row items-center gap-3`}>
						<View style={tw`rounded-md bg-mobile-highlight p-1.5`}>
							<Text
								style={tw`text-left text-xs font-bold text-ink-dull`}
								numberOfLines={1}
							>
								{`${byteSize(location.size_in_bytes)}`}
							</Text>
						</View>
						<Pressable onPress={() => modalRef.current?.present()}>
							<DotsThreeOutlineVertical
								weight="fill"
								size={20}
								color={tw.color('ink-dull')}
							/>
						</Pressable>
					</View>
				</Card>
			</Swipeable>
			<LocationModal
				editLocation={() => {
					editLocation();
					modalRef.current?.close();
				}}
				locationId={location.id}
				ref={modalRef}
			/>
		</Pressable>
	);
};

export default LocationItem;
