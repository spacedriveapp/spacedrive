import React from 'react';
import { Image, Text, View } from 'react-native';

import { FadeInUpAnimation, LogoAnimation } from '../../components/animation';
import { AnimatedButton } from '../../components/base/Button';
import { setItemToStorage } from '../../lib/storage';
import tw from '../../lib/tailwind';
import { useOnboardingStore } from '../../stores/useOnboardingStore';
import { OnboardingStackScreenProps } from '../../types/navigation';

const OnboardingScreen = ({ navigation }: OnboardingStackScreenProps<'Onboarding'>) => {
	const { hideOnboarding } = useOnboardingStore();

	function onButtonPress() {
		// Persist onboarding state only when app in production for now.
		if (process.env.NODE_ENV === 'production') {
			setItemToStorage('@onboarding', '1');
		}
		hideOnboarding();
	}

	return (
		<View style={tw`flex-1 items-center justify-around bg-black p-4 z-10`}>
			{/* Logo */}
			<LogoAnimation>
				<View style={tw`items-center mt-2`}>
					{/* TODO: Change this to @sd/assets. */}
					<Image source={require('../../assets/temp/logo.png')} style={tw`w-24 h-24`} />
				</View>
			</LogoAnimation>
			{/* Text */}
			<View style={tw``}>
				<FadeInUpAnimation delay={500}>
					<Text style={tw`text-white text-center text-5xl font-black leading-tight`}>
						A file explorer from the future.
					</Text>
				</FadeInUpAnimation>
				<FadeInUpAnimation delay={800}>
					<Text style={tw`text-gray-450 text-center px-6 mt-8 text-base leading-relaxed`}>
						Combine your drives and clouds into one database that you can organize and explore from
						any device.
					</Text>
				</FadeInUpAnimation>
			</View>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200}>
				<AnimatedButton variant="primary" onPress={onButtonPress}>
					<Text style={tw`text-white text-center px-6 py-2 text-base font-medium`}>
						Get Started
					</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</View>
	);
};

export default OnboardingScreen;
