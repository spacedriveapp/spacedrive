import { Image, Text, View } from 'react-native';
import { FadeInUpAnimation, LogoAnimation } from '~/components/animation/layout';
import { AnimatedButton } from '~/components/primitive/Button';
import tw from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const OnboardingScreen = ({ navigation }: OnboardingStackScreenProps<'Onboarding'>) => {
	return (
		<View style={tw`z-10 flex-1 items-center justify-around bg-app p-4`}>
			{/* Logo */}
			<LogoAnimation>
				<View style={tw`mt-2 items-center`}>
					<Image source={require('@sd/assets/images/logo.png')} style={tw`h-24 w-24`} />
				</View>
			</LogoAnimation>
			{/* Text */}
			<View>
				<FadeInUpAnimation delay={500}>
					<Text style={tw`text-center text-5xl font-black leading-tight text-ink`}>
						A file explorer from the future.
					</Text>
				</FadeInUpAnimation>
				<FadeInUpAnimation delay={800}>
					<Text style={tw`mt-8 px-6 text-center text-base leading-relaxed text-ink-dull`}>
						Combine your drives and clouds into one database that you can organize and explore from
						any device.
					</Text>
				</FadeInUpAnimation>
			</View>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200}>
				<AnimatedButton variant="accent" onPress={() => navigation.navigate('CreateLibrary')}>
					<Text style={tw`px-6 py-2 text-center text-base font-medium text-ink`}>Get Started</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</View>
	);
};

export default OnboardingScreen;
