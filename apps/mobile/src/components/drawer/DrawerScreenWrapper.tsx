import { useDrawerProgress } from '@react-navigation/drawer';
import React from 'react';
import Animated, { Extrapolate, interpolate, useAnimatedStyle } from 'react-native-reanimated';
import { SafeAreaView } from 'react-native-safe-area-context';

import tw from '../../lib/tailwind';

const DrawerScreenWrapper: React.FC = ({ children }) => {
	const progress: any = useDrawerProgress();

	const style = useAnimatedStyle(() => {
		const scale = interpolate(progress.value, [0, 1], [1, 0.88], Extrapolate.CLAMP);
		const translateX = interpolate(progress.value, [0, 1], [0, -12], Extrapolate.CLAMP);
		const translateY = interpolate(progress.value, [0, 1], [0, 12], Extrapolate.CLAMP);
		const borderRadius = interpolate(progress.value, [0, 1], [0, 16], Extrapolate.CLAMP);
		return {
			transform: [{ scale }, { translateX }, { translateY }],
			borderRadius
		};
	}, []);

	return (
		<Animated.View style={[tw.style('flex-1 bg-[#121219]'), style]}>
			<SafeAreaView>{children}</SafeAreaView>
		</Animated.View>
	);
};

export default DrawerScreenWrapper;
