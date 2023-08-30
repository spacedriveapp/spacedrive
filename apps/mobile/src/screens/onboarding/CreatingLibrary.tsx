import React from 'react';
import { Text } from 'react-native';
import { PulseAnimation } from '~/components/animation/lottie';
import { tw } from '~/lib/tailwind';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const CreatingLibraryScreen = () => {
	return (
		<OnboardingContainer>
			<Text style={tw`mb-4 text-5xl`}>🛠</Text>
			<OnboardingTitle>Creating your library</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>Creating your library...</OnboardingDescription>
			<PulseAnimation style={tw`mt-2 h-10`} speed={0.3} />
		</OnboardingContainer>
	);
};

export default CreatingLibraryScreen;
