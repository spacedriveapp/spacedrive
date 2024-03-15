import { Pen, Trash } from 'phosphor-react-native';
import { Animated, Pressable, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import { Location } from '@sd/client';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

import DeleteLocationModal from '../modal/confirmModals/DeleteLocationModal';

interface Props {
	progress: Animated.AnimatedInterpolation<number>;
	swipeable: Swipeable;
	location: Location;
	navigation: SettingsStackScreenProps<'LocationSettings'>['navigation'];
}

const RightActions = ({ progress, swipeable, location, navigation }: Props) => {
	const translate = progress.interpolate({
		inputRange: [0, 1],
		outputRange: [100, 0],
		extrapolate: 'clamp'
	});

	return (
		<Animated.View
			style={[
				tw`mr-3 flex flex-row items-center gap-2`,
				{ transform: [{ translateX: translate }] }
			]}
		>
			<Pressable
				style={tw`items-center justify-center rounded-md border border-app-lightborder bg-app-button px-3 py-1.5 shadow-sm`}
				onPress={() => {
					navigation.navigate('EditLocationSettings', { id: location.id });
					swipeable.close();
				}}
			>
				<Pen size={18} color="white" />
			</Pressable>
			<DeleteLocationModal
				locationId={location.id}
				trigger={
					<View
						style={tw`items-center justify-center rounded-md border border-app-lightborder bg-app-button px-3 py-1.5 shadow-sm`}
					>
						<Trash size={18} color="white" />
					</View>
				}
			/>
		</Animated.View>
	);
};

export default RightActions;
