import { useNavigation } from '@react-navigation/native';
import { DotsThreeVertical } from 'phosphor-react-native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { arraysEqual, humanizeSize, Location, useOnlineLocations } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

import FolderIcon from '../icons/FolderIcon';
import Card from '../layout/Card';
import RightActions from './RightActions';

interface ListLocationProps {
	location: Location;
}

const ListLocation = ({ location }: ListLocationProps) => {
	const swipeRef = useRef<Swipeable>(null);

	const navigation = useNavigation<SettingsStackScreenProps<'LocationSettings'>['navigation']>();
	const onlineLocations = useOnlineLocations();
	const online = onlineLocations.some((l) => arraysEqual(location.pub_id, l));

	return (
		<Swipeable
			ref={swipeRef}
			containerStyle={tw`h-16 rounded-md border border-app-cardborder bg-app-card`}
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
				<View style={tw`w-1/2 flex-row items-center gap-2`}>
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
				<View style={tw`flex-row items-center gap-1.5`}>
					<View style={tw`rounded-md border border-app-box/70 bg-app/70 px-1.5 py-1`}>
						<Text style={tw`text-xs font-bold text-ink-dull`} numberOfLines={1}>
							{`${humanizeSize(location.size_in_bytes)}`}
						</Text>
					</View>
					<Pressable hitSlop={24} onPress={() => swipeRef.current?.openRight()}>
						<DotsThreeVertical weight="bold" size={20} color={tw.color('ink-dull')} />
					</Pressable>
				</View>
			</Card>
		</Swipeable>
	);
};

export default ListLocation;
