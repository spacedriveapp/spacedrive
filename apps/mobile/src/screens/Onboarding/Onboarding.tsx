import React from 'react';
import { Image, Text, View } from 'react-native';

import { FadeInUpAnimation, LogoAnimation } from '../../components/animation';
import tw from '../../lib/tailwind';
import { OnboardingStackScreenProps } from '../../types/navigation';

const OnboardingScreen = ({ navigation }: OnboardingStackScreenProps<'Onboarding'>) => {
	return (
		<View style={tw`flex-1 items-center justify-around bg-black p-4 z-10`}>
			<LogoAnimation>
				<View style={tw`items-center mt-2`}>
					{/* TODO: Change this to @sd/assets when available. */}
					<Image source={require('../../assets/images/logo.png')} style={tw`w-24 h-24`} />
				</View>
			</LogoAnimation>
			<View style={tw``}>
				<FadeInUpAnimation delay={500}>
					<Text style={tw`text-white text-center text-5xl font-black leading-tight`}>
						A file explorer from the future.
					</Text>
				</FadeInUpAnimation>
				<FadeInUpAnimation delay={800}>
					<Text style={tw`text-gray-450 text-center px-6 mt-8 text-base leading-2`}>
						Combine your drives and clouds into one database that you can organize and explore from
						any device.
					</Text>
				</FadeInUpAnimation>
			</View>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200}>
				<View style={tw`bg-primary-600 rounded-md shadow-sm`}>
					<Text style={tw`text-white text-center px-6 py-2 text-base font-medium`}>
						Get Started
					</Text>
				</View>
			</FadeInUpAnimation>
		</View>
	);
};

export default OnboardingScreen;
