import { Globe } from 'phosphor-react-native';
import React from 'react';
import { Image, Linking, Platform, Text, View } from 'react-native';
import { useBridgeQuery } from '@sd/client';
import { DiscordIcon, GitHubIcon } from '~/components/icons/Brands';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { tw } from '~/lib/tailwind';

const AboutScreen = () => {
	const buildInfo = useBridgeQuery(['buildInfo']);

	return (
		<View style={tw.style('flex-1 p-5')}>
			<View style={tw.style('flex flex-row items-center')}>
				<Image
					source={require('../../../../assets/icon.png')}
					style={tw.style('mr-8 h-[88px] w-[88px] rounded-3xl')}
					resizeMode="contain"
				/>
				<View style={tw.style('flex flex-col')}>
					<Text style={tw.style('text-2xl font-bold text-white')}>
						Spacedrive{' '}
						{`for ${
							Platform.OS === 'android'
								? Platform.OS[0]?.toUpperCase() + Platform.OS.slice(1)
								: Platform.OS[0] + Platform.OS.slice(1).toUpperCase()
						}`}
					</Text>
					<Text style={tw.style('mt-1 text-sm text-ink-dull')}>
						The file manager from the future.
					</Text>
					<Text style={tw.style('mt-1 text-xs text-ink-faint/80')}>
						v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
					</Text>
				</View>
			</View>
			{/* iOS has buttons falling out of the screen for some reason. So, I made the buttons veritical instead */}
			<View style={tw.style('my-5 flex-col justify-between gap-2')}>
				{/* Discord Button */}
				<Button
					onPress={() => Linking.openURL('https://discord.gg/ukRnWSnAbG')}
					style={tw.style('flex-row items-center')}
					variant="gray"
				>
					<View style={tw.style('h-4 w-4')}>
						<DiscordIcon fill="white" />
					</View>
					<Text style={tw.style('ml-2 text-white')}>Join Discord</Text>
				</Button>

				{/* GitHub Button */}
				<Button
					onPress={() => Linking.openURL('https://github.com/spacedriveapp/spacedrive')}
					style={tw.style('flex-row items-center')}
					variant="accent"
				>
					<View style={tw.style('h-4 w-4')}>
						<GitHubIcon fill="white" />
					</View>
					<Text style={tw.style('ml-2 text-white')}>Star on GitHub</Text>
				</Button>

				{/* Website Button */}
				<Button
					onPress={() => Linking.openURL('https://spacedrive.app')}
					style={tw.style('flex-row items-center')}
					variant="accent"
				>
					<View style={tw.style('h-4 w-4')}>
						<Globe size={16} color="white" />
					</View>
					<Text style={tw.style('ml-2 text-white')}>Website</Text>
				</Button>
			</View>
			<Divider />
			<View style={tw.style('my-5')}>
				<Text style={tw.style('mb-3 text-lg font-bold text-ink')}>Vision</Text>
				<Text style={tw.style('w-full text-sm text-ink-faint')}>
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
				<Text style={tw.style('my-5 text-lg font-bold text-ink')}>
					Meet the contributors behind Spacedrive
				</Text>
				{/* For some reason, it won't load. ¯\_(ツ)_/¯ */}
				<Image
					source={{
						uri: 'https://contrib.rocks/image?repo=spacedriveapp/spacedrive&columns=12&anon=1'
					}}
					style={{ height: 200, width: '100%' }}
					resizeMode="contain"
				/>
			</View>
		</View>
	);
};

export default AboutScreen;

// React Native doesn't allow for SVGs to be imported, so we have to use react-native-svg.
