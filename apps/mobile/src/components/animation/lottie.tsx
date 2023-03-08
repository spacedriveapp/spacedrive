import AnimatedLottieView, { AnimatedLottieViewProps } from 'lottie-react-native';

type AnimationProps = Omit<AnimatedLottieViewProps, 'source'>;

export const PulseAnimation = ({ style }: AnimationProps) => {
	return (
		<AnimatedLottieView
			autoPlay
			loop
			source={require('@sd/assets/lottie/loading-pulse.json')}
			style={style}
		/>
	);
};
