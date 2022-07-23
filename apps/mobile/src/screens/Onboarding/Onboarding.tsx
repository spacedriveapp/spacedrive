import React from 'react';
import { Image, Text, View } from 'react-native';

import { FadeInUpAnimation } from '../../components/animation';
import tw from '../../lib/tailwind';
import { OnboardingStackScreenProps } from '../../types/navigation';

const OnboardingScreen = ({ navigation }: OnboardingStackScreenProps<'Onboarding'>) => {
	return (
		<View style={tw`flex-1 items-center justify-around bg-black p-4`}>
			{/* TODO: Gradient Image background with balls :) */}
			<FadeInUpAnimation delay={100}>
				<View style={tw`items-center mt-2`}>
					{/* TODO: Change this to @sd/assets when available. */}
					<Image source={require('../../assets/images/logo.png')} style={tw`w-24 h-24`} />
					{/* <Text style={tw`text-white font-bold text-center text-3xl mt-4`}>Spacedrive</Text> */}
				</View>
			</FadeInUpAnimation>
			<View style={tw``}>
				<FadeInUpAnimation delay={400}>
					<Text style={tw`text-white text-center text-5xl font-black leading-tight`}>
						A file explorer from the future.
					</Text>
				</FadeInUpAnimation>
				<FadeInUpAnimation delay={800}>
					<Text style={tw`text-gray-300 text-center px-6 mt-8 text-base leading-snug`}>
						Combine your drives and clouds into one database that you can organize and explore from
						any device.
					</Text>
				</FadeInUpAnimation>
			</View>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200}>
				<View style={tw`bg-primary-500 rounded-full`}>
					<Text style={tw`text-white text-center px-8 py-4 font-bold`}>Get Started</Text>
				</View>
			</FadeInUpAnimation>
		</View>
	);
};

export default OnboardingScreen;
