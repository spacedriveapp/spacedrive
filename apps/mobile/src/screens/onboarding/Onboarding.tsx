import { Image, Text, View } from 'react-native';
import { FadeInUpAnimation, LogoAnimation } from '~/components/animation/layout';
import { AnimatedButton } from '~/components/primitive/Button';
import tw from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const OnboardingScreen = ({ navigation }: OnboardingStackScreenProps<'Onboarding'>) => {
	return (
		<View style={tw`bg-app z-10 flex-1 items-center justify-around p-4`}>
			{/* Logo */}
			<LogoAnimation>
				<View style={tw`mt-2 items-center`}>
					<Image source={require('@sd/assets/images/logo.png')} style={tw`h-24 w-24`} />
				</View>
			</LogoAnimation>
			{/* Text */}
			<View>
				<FadeInUpAnimation delay={500}>
					<Text style={tw`text-ink text-center text-5xl font-black leading-tight`}>
						A file explorer from the future.
					</Text>
				</FadeInUpAnimation>
				<FadeInUpAnimation delay={800}>
					<Text style={tw`text-ink-dull mt-8 px-6 text-center text-base leading-relaxed`}>
						Combine your drives and clouds into one database that you can organize and explore from
						any device.
					</Text>
				</FadeInUpAnimation>
			</View>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200}>
				<AnimatedButton variant="accent" onPress={() => navigation.navigate('CreateLibrary')}>
					<Text style={tw`text-ink px-6 py-2 text-center text-base font-medium`}>Get Started</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</View>
	);
};

export default OnboardingScreen;
