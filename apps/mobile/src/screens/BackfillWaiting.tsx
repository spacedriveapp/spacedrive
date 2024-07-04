import { useNavigation } from '@react-navigation/native';
import { AppLogo } from '@sd/assets/images';
import { Image } from 'expo-image';
import React, { useEffect } from 'react';
import { Dimensions, Text, View } from 'react-native';
import Animated, {
	Easing,
	useAnimatedStyle,
	useSharedValue,
	withRepeat,
	withTiming
} from 'react-native-reanimated';
import { Circle, Defs, RadialGradient, Stop, Svg } from 'react-native-svg';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

const { width } = Dimensions.get('window');

const BackfillWaiting = () => {
	const animation = useSharedValue(0);
	const navigation = useNavigation();

	useEffect(() => {
		animation.value = withRepeat(
			withTiming(1, { duration: 5000, easing: Easing.inOut(Easing.ease) }),
			-1,
			true
		);
	}, [animation]);

	const animatedStyle = useAnimatedStyle(() => {
		return {
			opacity: animation.value
		};
	});

	const enableSync = useLibraryMutation(['sync.backfill'], {
		onSuccess: () => {
			syncEnabled.refetch();
			navigation.navigate('Root', {
				screen: 'Home',
				params: {
					screen: 'SettingsStack',
					params: {
						screen: 'SyncSettings'
					}
				}
			});
		}
	});

	const syncEnabled = useLibraryQuery(['sync.enabled']);

	useEffect(() => {
		(async () => {
			await enableSync.mutateAsync(null);
		})();
	}, []);

	return (
		<View style={tw`flex-1 items-center justify-center bg-black`}>
			<Animated.View
				style={[
					twStyle(`absolute items-center justify-center`, {
						width: width * 2,
						height: width * 2,
						borderRadius: (width * 0.8) / 2
					}),
					animatedStyle
				]}
			>
				<Svg height="100%" width="100%" viewBox="0 0 100 100">
					<Defs>
						<RadialGradient id="grad" cx="50%" cy="50%" r="50%" fx="50%" fy="50%">
							<Stop offset="0%" stopColor="#4B0082" stopOpacity="1" />
							<Stop offset="100%" stopColor="#000000" stopOpacity="0" />
						</RadialGradient>
					</Defs>
					<Circle cx="50" cy="50" r="50" fill="url(#grad)" />
				</Svg>
			</Animated.View>
			<Image source={AppLogo} style={tw`mb-4 h-[100px] w-[100px]`} />
			<Text style={tw`mx-10 mb-4 text-center text-md leading-6 text-ink`}>
				Library is being backfilled right now for Sync!
				<Text style={tw`font-bold`}> Please hold </Text>
				while this process takes place.
			</Text>
		</View>
	);
};

export default BackfillWaiting;
