import { ArrowLeft, Globe } from 'phosphor-react-native';
import { FontAwesome5, FontAwesome, Ionicons } from '@expo/vector-icons';
import React, { useEffect } from 'react';
import { Image, Linking, Platform, Text, View } from 'react-native';
import { Divider } from '~/components/primitive/Divider';
import { tw } from '~/lib/tailwind';
import { Button } from '~/components/primitive/Button';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AboutScreen = ({ navigation }: SettingsStackScreenProps<'About'>) => {
	useEffect(() => {
		navigation.setOptions({
			headerBackImage: () => (
				<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
			)
		});
	});

	return (
		<View style={{ flex: 1, padding: 20 }}>
			<View style={{ flexDirection: 'row', alignItems: 'center' }}>
				<Image
					source={require('../../../../assets/icon.png')}
					style={{ marginRight: 8, height: 88, width: 88, borderRadius: 20, overflow: 'hidden' }}
					resizeMode="contain"
				/>
				<View style={{ flex: 1, flexDirection: 'column' }}>
					<Text style={{ fontSize: 20, fontWeight: 'bold', color: 'white' }}>
						Spacedrive {`for ${Platform.OS === 'android' ? Platform.OS[0]?.toUpperCase() + Platform.OS.slice(1) : Platform.OS[0] + Platform.OS.slice(1).toUpperCase()}`}
					</Text>
					<Text style={{ marginTop: 1, fontSize: 14, color: 'grey' }}>
						The file manager from the future.
					</Text>
					<Text style={{ marginTop: 1, fontSize: 12, color: 'grey' }}>
						v0.1.0 {/* Replace with Build Info at some point */}
					</Text>
				</View>
			</View>
			{/* iOS has the Website button jamming into themselves for some reason. */}
			<View style={{ marginTop: 5, flexDirection: 'row', justifyContent: 'space-evenly' }}>
				{/* Discord Button */}
				<Button
					onPress={() => Linking.openURL('https://discord.gg/ukRnWSnAbG')}
					style={{ flexDirection: 'row', alignItems: 'center' }}
					variant="gray"
				>
					<View style={{ height: 16, width: 16 }}>
						<View>
							<FontAwesome5 name="discord" size={16} color="white" />
						</View>
					</View>
					<Text style={{ marginLeft: 5, color: 'white' }}>Join Discord</Text>
				</Button>

				{/* GitHub Button */}
				<Button
					onPress={() => Linking.openURL('https://github.com/spacedriveapp/spacedrive')}
					style={{ flexDirection: 'row', alignItems: 'center' }}
					variant="accent"
				>
					<View style={{ height: 16, width: 16 }}>
						<View>
							<FontAwesome name="github" size={16} color="white" />
						</View>
					</View>
					<Text style={{ marginLeft: 5, color: 'white' }}>Star on GitHub</Text>
				</Button>

				{/* Website Button */}
				<Button
					onPress={() => Linking.openURL('https://spacedrive.app')}
					style={{ flexDirection: 'row', alignItems: 'center' }}
					variant="accent"
				>
					<View style={{ height: 16, width: 16 }}>
						<View>
							<Ionicons name="ios-globe-outline" size={16} color="white" />
						</View>
					</View>
					<Text style={{ marginLeft: 5, color: 'white' }}>Website</Text>
				</Button>
			</View>
			{/* Divider Component */}
			<View style={{ paddingVertical: 10 }}>
				<Divider />
			</View>
			<View style={{ marginTop: 5 }}>
				<Text style={{ fontSize: 16, fontWeight: 'bold', color: 'white', paddingBottom: 5 }}>Vision</Text>
				<Text style={{ marginTop: 3, fontSize: 12, color: 'grey', lineHeight: 20 }}>
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
			{/* Divider Component */}
			<View style={{ paddingVertical: 10 }}>
				<Divider />
			</View>
			<View>
				<Text style={{ marginTop: 5, fontSize: 16, fontWeight: 'bold', color: 'white', paddingBottom: 10 }}>
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
