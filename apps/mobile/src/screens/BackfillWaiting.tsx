/* eslint-disable react-hooks/exhaustive-deps */
import { useNavigation } from '@react-navigation/native';
import { AppLogo } from '@sd/assets/images';
import { Image } from 'expo-image';
import React, { useEffect } from 'react';
import { Dimensions, StyleSheet, Text, View } from 'react-native';
import Animated, {
	Easing,
	useAnimatedStyle,
	useSharedValue,
	withRepeat,
	withTiming
} from 'react-native-reanimated';
import { Circle, Defs, RadialGradient, Stop, Svg } from 'react-native-svg';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { BackfillWaitingStackScreenProps } from '~/navigation/BackfillWaitingStack';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

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

	const enableSync = useLibraryMutation(['sync.backfill'], {});

	useEffect(() => {
		async function _() {
			await enableSync.mutateAsync(null).then(() =>
				navigation.navigate('Root', {
					screen: 'Home',
					params: {
						screen: 'SettingsStack',
						params: {
							screen: 'SyncSettings'
						}
					}
				})
			);
		}

		_();
	}, []);

	return (
		<View style={styles.container}>
			<Animated.View style={[styles.gradientContainer, animatedStyle]}>
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
			<Image source={AppLogo} style={styles.icon} />
			<Text style={styles.text}>
				Library is being backfilled right now for Sync!
				<Text style={styles.boldText}> Please hold </Text>
				while this process takes place.
			</Text>
		</View>
	);
};

const styles = StyleSheet.create({
	container: {
		flex: 1,
		backgroundColor: '#000000', // Black background
		alignItems: 'center',
		justifyContent: 'center'
	},
	gradientContainer: {
		position: 'absolute',
		width: width * 2, // Adjust the size of the circular gradient
		height: width * 2, // Keep the aspect ratio to make it circular
		borderRadius: (width * 0.8) / 2,
		alignItems: 'center',
		justifyContent: 'center'
	},
	icon: {
		width: 100,
		height: 100,
		marginBottom: 20
	},
	text: {
		color: '#FFFFFF',
		textAlign: 'center',
		marginHorizontal: 40,
		marginBottom: 20,
		fontSize: 16,
		lineHeight: 24
	},
	boldText: {
		fontWeight: 'bold'
	}
});

export default BackfillWaiting;
