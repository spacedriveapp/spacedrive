import { Image, Text, View } from 'react-native';
import { FadeInUpAnimation, LogoAnimation } from '~/components/animation/layout';
import { AnimatedButton } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

export function OnboardingContainer({ children }: React.PropsWithChildren) {
	return (
		<View style={tw`flex-1`}>
			<View style={tw`z-10 flex-1 items-center justify-center px-4`}>
				{children}
				<Text style={tw`text-ink-dull/50 absolute bottom-8 text-xs`}>
					&copy; 2022 Spacedrive Technology Inc.
				</Text>
			</View>
			{/* Bloom */}
			<Image
				source={require('@sd/assets/images/bloom-one.png')}
				style={tw`top-100 absolute h-screen w-screen opacity-20`}
			/>
		</View>
	);
}

const GetStartedScreen = ({ navigation }: OnboardingStackScreenProps<'GetStarted'>) => {
	return (
		<OnboardingContainer>
			{/* Logo */}
			<LogoAnimation style={tw`items-center`}>
				<Image source={require('@sd/assets/images/logo.png')} style={tw`h-30 w-30`} />
			</LogoAnimation>
			{/* Title */}
			<FadeInUpAnimation delay={500} style={tw`mt-8`}>
				<Text style={tw`text-ink text-center text-4xl font-extrabold leading-tight`}>
					The file explorer from the future.
				</Text>
			</FadeInUpAnimation>
			{/* Description */}
			<FadeInUpAnimation delay={800} style={tw`mt-8`}>
				<Text style={tw`text-ink-dull px-6 text-center text-base leading-relaxed`}>
					Welcome to Spacedrive, an open source cross-platform file manager.
				</Text>
			</FadeInUpAnimation>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200} style={tw`mt-8`}>
				<AnimatedButton variant="accent" size="md" onPress={() => navigation.push('NewLibrary')}>
					<Text style={tw`text-ink text-center text-base font-medium`}>Get Started</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</OnboardingContainer>
	);
};

export default GetStartedScreen;
