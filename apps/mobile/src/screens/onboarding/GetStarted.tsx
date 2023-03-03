import { useNavigation } from '@react-navigation/native';
import { CaretLeft } from 'phosphor-react-native';
import { Image, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { FadeInUpAnimation, LogoAnimation } from '~/components/animation/layout';
import { AnimatedButton } from '~/components/primitive/Button';
import { styled, tw, twStyle } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

export function OnboardingContainer({ children }: React.PropsWithChildren) {
	const navigation = useNavigation();

	const { top } = useSafeAreaInsets();

	return (
		<View style={tw`flex-1`}>
			{/* NOTE: Might be buggy, this doesn't re-render when result changes. Works fine atm though. */}
			{navigation.canGoBack() && (
				<Pressable
					style={twStyle('absolute left-6 z-50', { top: top + 16 })}
					onPress={() => navigation.goBack()}
				>
					<CaretLeft size={24} weight="bold" color="white" />
				</Pressable>
			)}
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

export const OnboardingTitle = styled(
	Text,
	'text-ink text-center text-4xl font-extrabold leading-tight'
);

export const OnboardingDescription = styled(
	Text,
	'text-ink-dull text-center text-base leading-relaxed'
);

const GetStartedScreen = ({ navigation }: OnboardingStackScreenProps<'GetStarted'>) => {
	return (
		<OnboardingContainer>
			{/* Logo */}
			<LogoAnimation style={tw`items-center`}>
				<Image source={require('@sd/assets/images/logo.png')} style={tw`h-30 w-30`} />
			</LogoAnimation>
			{/* Title */}
			<FadeInUpAnimation delay={500} style={tw`mt-8`}>
				<OnboardingTitle>The file explorer from the future.</OnboardingTitle>
			</FadeInUpAnimation>
			{/* Description */}
			<FadeInUpAnimation delay={800} style={tw`mt-8`}>
				<OnboardingDescription style={tw`px-4`}>
					Welcome to Spacedrive, an open source cross-platform file manager.
				</OnboardingDescription>
			</FadeInUpAnimation>
			{/* Get Started Button */}
			<FadeInUpAnimation delay={1200} style={tw`mt-8`}>
				<AnimatedButton variant="accent" onPress={() => navigation.push('NewLibrary')}>
					<Text style={tw`text-ink text-center text-base font-medium`}>Get Started</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</OnboardingContainer>
	);
};

export default GetStartedScreen;
