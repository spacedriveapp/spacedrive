import { ArrowLeft, Globe } from 'phosphor-react-native';
import React, { useEffect } from 'react';
import { Image, Linking, Platform, Text, View } from 'react-native';
import { Divider } from '~/components/primitive/Divider';
import { tw } from '~/lib/tailwind';
import { Button } from '~/components/primitive/Button';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';
import Svg, { Path, SvgProps } from "react-native-svg"
import { useBridgeQuery } from '@sd/client';

const AboutScreen = ({ navigation }: SettingsStackScreenProps<'About'>) => {
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
						Spacedrive {`for ${Platform.OS === 'android' ? Platform.OS[0]?.toUpperCase() + Platform.OS.slice(1) : Platform.OS[0] + Platform.OS.slice(1).toUpperCase()}`}
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
					variant='gray'
				>
					<View style={tw.style('h-4 w-4')}>
						<DiscordRN fill="white" />
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
						<GitHubRN fill="white" />
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
						uri:
							'https://contrib.rocks/image?repo=spacedriveapp/spacedrive&columns=12&anon=1',
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
const DiscordRN = (props: SvgProps) => (
	<Svg viewBox="0 0 24 24" {...props}>
		<Path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418Z" />
	</Svg>
)

const GitHubRN = (props: SvgProps) => (
	<Svg viewBox="0 0 24 24" {...props}>
		<Path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
	</Svg>
)
