import * as Haptics from 'expo-haptics';
import { ArrowRight } from 'phosphor-react-native';
import React from 'react';
import { Controller } from 'react-hook-form';
import { Linking, Pressable, Text, View, ViewStyle } from 'react-native';
import { useOnboardingContext } from '~/components/context/OnboardingContext';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

type RadioButtonProps = {
	title: string;
	description: string;
	isSelected: boolean;
	style?: ViewStyle;
};

// Make this a component?
const RadioButton = ({ title, description, isSelected, style }: RadioButtonProps) => {
	return (
		<View
			style={twStyle(
				'flex w-full flex-row items-center rounded-md border border-app-line bg-app-box/50 p-3',
				style
			)}
		>
			<View
				style={twStyle(
					'mr-2.5 h-5 w-5 items-center justify-center rounded-full',
					isSelected ? 'bg-accent' : 'bg-gray-900'
				)}
			>
				{isSelected && <View style={tw`h-1.5 w-1.5 rounded-full bg-white`} />}
			</View>
			<View style={tw`flex-1`}>
				<Text style={tw`text-base font-bold text-ink`}>{title}</Text>
				<Text style={tw`text-sm text-ink-faint`}>{description}</Text>
			</View>
		</View>
	);
};

const PrivacyScreen = () => {
	const { forms, submit } = useOnboardingContext();

	const form = forms.useForm('Privacy');

	return (
		<OnboardingContainer>
			<OnboardingTitle>Your Privacy</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>
				Spacedrive is built for privacy, that's why we're open source and local first. So
				we'll make it very clear what data is shared with us.
			</OnboardingDescription>
			<View style={tw`w-full`}>
				<Controller
					name="shareTelemetry"
					control={form.control}
					render={({ field: { onChange, value } }) => (
						<>
							<Pressable onPress={() => onChange('full')}>
								<RadioButton
									title="Share anonymous usage data"
									description="This give us a completely anonymous picture of how you use Spacedrive."
									isSelected={value === 'full'}
									style={tw`mb-3 mt-4`}
								/>
							</Pressable>
							<Pressable testID="share-minimal" onPress={() => onChange('minimal')}>
								<RadioButton
									title="Share minimal data"
									description="This just tells us how many people use Spacedrive and device/version details."
									isSelected={value === 'minimal'}
								/>
							</Pressable>
							<Pressable testID="share-none" onPress={() => onChange('none')}>
								<RadioButton
									title="Don't share anything"
									description="Sends absolutely no analytics data from the Spacedrive app."
									isSelected={value === 'none'}
								/>
							</Pressable>
						</>
					)}
				/>
			</View>
			<Button
				variant="accent"
				size="sm"
				onPress={() => {
					Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
					form.handleSubmit(submit)();
				}}
				style={tw`mt-6`}
			>
				<Text style={tw`text-center text-base font-medium text-ink`}>Continue</Text>
			</Button>
			<Pressable
				onPress={() => {
					Linking.openURL('https://www.spacedrive.com/docs/product/resources/privacy');
				}}
				style={tw`mt-6 flex flex-row items-center justify-center`}
			>
				<ArrowRight size={16} style={tw`mr-0.5`} color={tw.color('text-ink-faint')} />
				<Text style={tw`text-center text-sm font-medium text-ink-faint underline`}>
					Learn more about the data we collect
				</Text>
			</Pressable>
		</OnboardingContainer>
	);
};

export default PrivacyScreen;
