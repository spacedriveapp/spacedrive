import LottieView from 'lottie-react-native';
// They probably forgot to export the type on this update lol.
// import type LottieViewProps from 'lottie-react-native';
import { StyleProp, ViewStyle } from 'react-native';

type AnimationProps = {
	style?: StyleProp<ViewStyle>;
	speed?: number;
};

export const PulseAnimation = (props: AnimationProps) => {
	return (
		<LottieView
			autoPlay
			loop
			source={require('@sd/assets/lottie/loading-pulse.json')}
			{...props}
		/>
	);
};
