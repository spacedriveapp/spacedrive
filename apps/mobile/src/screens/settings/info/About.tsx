import { Image } from 'expo-image';
import { Globe } from 'phosphor-react-native';
import React from 'react';
import { Linking, Platform, Text, View } from 'react-native';
import { useBridgeQuery } from '@sd/client';
import { DiscordIcon, GitHubIcon } from '~/components/icons/Brands';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { tw } from '~/lib/tailwind';

const AboutScreen = () => {
	const buildInfo = useBridgeQuery(['buildInfo']);

	return (
		<ScreenContainer style={tw`justify-start gap-0 px-6`}>
			<View style={tw`flex flex-row items-center`}>
				<Image
					source={require('../../../../assets/icon.png')}
					style={tw`mr-8 h-[88px] w-[88px] rounded-3xl`}
					resizeMode="contain"
				/>
				<View style={tw`flex flex-col`}>
					<Text style={tw`text-2xl font-bold text-white`}>
						Spacedrive{' '}
						{`for ${
							Platform.OS === 'android'
								? Platform.OS[0]?.toUpperCase() + Platform.OS.slice(1)
								: Platform.OS[0] + Platform.OS.slice(1).toUpperCase()
						}`}
					</Text>
					<Text style={tw`mt-1 text-sm text-ink-dull`}>
						The file manager from the future.
					</Text>
					<Text style={tw`mt-1 text-xs text-ink-faint/80`}>
						v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
					</Text>
				</View>
			</View>
			{/* iOS has buttons falling out of the screen for some reason. So, I made the buttons veritical instead */}
			<View style={tw`my-5 flex-col justify-between gap-2`}>
				{/* Discord Button */}
				<Button
					onPress={() => Linking.openURL('https://discord.gg/ukRnWSnAbG')}
					style={tw`flex-row items-center`}
					variant="gray"
				>
					<View style={tw`h-4 w-4`}>
						<DiscordIcon fill="white" />
					</View>
					<Text style={tw`ml-2 font-bold text-white`}>Join Discord</Text>
				</Button>

				{/* GitHub Button */}
				<Button
					onPress={() => Linking.openURL('https://github.com/spacedriveapp/spacedrive')}
					style={tw`flex-row items-center font-bold`}
					variant="accent"
				>
					<View style={tw`h-4 w-4`}>
						<GitHubIcon fill="white" />
					</View>
					<Text style={tw`ml-2 font-bold text-white`}>Star on GitHub</Text>
				</Button>

				{/* Website Button */}
				<Button
					onPress={() => Linking.openURL('https://spacedrive.app')}
					style={tw`flex-row items-center`}
					variant="accent"
				>
					<View style={tw`h-4 w-4`}>
						<Globe weight="bold" size={16} color="white" />
					</View>
					<Text style={tw`ml-2 font-bold text-white`}>Website</Text>
				</Button>
			</View>
			<Divider />
			<View style={tw`my-5`}>
				<Text style={tw`mb-3 text-lg font-bold text-ink`}>Vision</Text>
				<Text style={tw`w-full text-sm text-ink-faint`}>
					Many of us have multiple cloud accounts, drives that aren’t backed up and data
					at risk of loss. We depend on cloud services like Google Photos and iCloud, but
					are locked in with limited capacity and almost zero interoperability between
					services and operating systems. Photo albums shouldn’t be stuck in a device
					ecosystem, or harvested for advertising data. They should be OS agnostic,
					permanent and personally owned. Data we create is our legacy, that will long
					outlive us—open source technology is the only way to ensure we retain absolute
					control over the data that defines our lives, at unlimited scale.
				</Text>
			</View>
			<Divider />
			<View>
				<Text style={tw`my-5 text-lg font-bold text-ink`}>
					Meet the contributors behind Spacedrive
				</Text>
				{/* TODO: Temporary image url approach until a solution is reached */}
				<Image
					source={{ uri: 'https://i.imgur.com/SwUcWHP.png' }}
					style={{ height: 200, width: '100%' }}
					contentFit="contain"
				/>
			</View>
		</ScreenContainer>
	);
};

export default AboutScreen;
