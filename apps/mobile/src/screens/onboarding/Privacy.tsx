import React, { useState } from 'react';
import { Pressable, Text, View, ViewStyle } from 'react-native';
import { getOnboardingStore } from '@sd/client';
import { Button } from '~/components/primitive/Button';
import { tw, twStyle } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
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

const PrivacyScreen = ({ navigation }: OnboardingStackScreenProps<'Privacy'>) => {
	const [shareTelemetry, setShareTelemetry] = useState<'share-telemetry' | 'no-share-telemetry'>(
		'share-telemetry'
	);

	const onPress = () => {
		getOnboardingStore().shareTelemetry = shareTelemetry === 'share-telemetry';
		navigation.navigate('CreatingLibrary');
	};

	return (
		<OnboardingContainer>
			<OnboardingTitle>Your Privacy</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>
				Spacedrive is built for privacy, that's why we're open source and local first. So
				we'll make it very clear what data is shared with us.
			</OnboardingDescription>
			<View style={tw`w-full`}>
				<Pressable onPress={() => setShareTelemetry('share-telemetry')}>
					<RadioButton
						title="Share anonymous usage"
						description="Share completely anonymous telemetry data to help the developers improve the app"
						isSelected={shareTelemetry === 'share-telemetry'}
						style={tw`mb-3 mt-4`}
					/>
				</Pressable>
				<Pressable
					testID="share-nothing"
					onPress={() => setShareTelemetry('no-share-telemetry')}
				>
					<RadioButton
						title="Share nothing"
						description="Do not share any telemetry data with the developers"
						isSelected={shareTelemetry === 'no-share-telemetry'}
					/>
				</Pressable>
			</View>
			<Button variant="accent" size="sm" onPress={onPress} style={tw`mt-6`}>
				<Text style={tw`text-center text-base font-medium text-ink`}>Continue</Text>
			</Button>
		</OnboardingContainer>
	);
};

export default PrivacyScreen;
