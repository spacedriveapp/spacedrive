import AnimatedLottieView from 'lottie-react-native';
import { StyleProp, View, ViewStyle } from 'react-native';

type Props = {
	style?: StyleProp<ViewStyle>;
};

export const PulseAnimation = ({ style }: Props) => {
	return (
		<View>
			<AnimatedLottieView
				autoPlay
				loop
				source={require('@sd/assets/lottie/loading-pulse.json')}
				style={style}
			/>
		</View>
	);
};
