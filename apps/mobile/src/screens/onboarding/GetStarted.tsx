import { useNavigation, useRoute } from '@react-navigation/native';
import { AppLogo, BloomOne } from '@sd/assets/images';
import { MotiView } from 'moti';
import { CaretLeft } from 'phosphor-react-native';
import { Image, KeyboardAvoidingView, Platform, Pressable, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { FadeInUpAnimation, LogoAnimation } from '~/components/animation/layout';
import { AnimatedButton } from '~/components/primitive/Button';
import { styled, tw, twStyle } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

export function OnboardingContainer({ children }: React.PropsWithChildren) {
	const navigation = useNavigation();
	const route = useRoute();

	const { top, bottom } = useSafeAreaInsets();

	return (
		<View style={tw`flex-1`}>
			{route.name !== 'GetStarted' && route.name !== 'CreatingLibrary' && (
				<Pressable
					style={twStyle('absolute left-6 z-50', { top: top + 16 })}
					onPress={() => navigation.goBack()}
				>
					<CaretLeft size={24} weight="bold" color="white" />
				</Pressable>
			)}
			<View style={tw`z-10 flex-1 items-center justify-center`}>
				<KeyboardAvoidingView
					behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
					keyboardVerticalOffset={bottom}
					style={tw`w-full flex-1 items-center justify-center`}
				>
					<MotiView style={tw`w-full items-center justify-center px-4`}>
						{children}
					</MotiView>
				</KeyboardAvoidingView>
				<Text style={tw`absolute bottom-8 text-xs text-ink-dull/50`}>
					&copy; {new Date().getFullYear()} Spacedrive Technology Inc.
				</Text>
			</View>
			{/* Bloom */}
			<Image source={BloomOne} style={tw`top-100 absolute h-screen w-screen opacity-20`} />
		</View>
	);
}

export const OnboardingTitle = styled(
	Animated.Text,
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
				<Image source={AppLogo} style={tw`h-30 w-30`} />
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
					<Text style={tw`text-center text-base font-medium text-ink`}>Get Started</Text>
				</AnimatedButton>
			</FadeInUpAnimation>
		</OnboardingContainer>
	);
};

export default GetStartedScreen;
