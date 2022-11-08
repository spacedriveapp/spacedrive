import { useNavigation } from '@react-navigation/native';
import {
	Books,
	FlyingSaucer,
	GearSix,
	HardDrive,
	Heart,
	Key,
	PaintBrush,
	PuzzlePiece,
	ShareNetwork,
	ShieldCheck,
	TagSimple
} from 'phosphor-react-native';
import React from 'react';
import { Pressable, SectionList, Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SettingsStackParamList, SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

interface SettingsItemType {
	title: string;
	icon: JSX.Element;
	navigateTo: keyof SettingsStackParamList;
}

interface SectionType {
	title: string;
	data: SettingsItemType[];
}

const sections: SectionType[] = [
	{
		title: 'Client',
		data: [
			{
				icon: <GearSix weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'GeneralSettings',
				title: 'General'
			},
			{
				icon: <Books weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'LibrarySettings',
				title: 'Libraries'
			},
			{
				icon: <PaintBrush weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'AppearanceSettings',
				title: 'Appearance'
			},
			{
				icon: <ShieldCheck weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'PrivacySettings',
				title: 'Privacy'
			},
			{
				icon: <PuzzlePiece weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'ExtensionsSettings',
				title: 'Extensions'
			}
		]
	},
	{
		title: 'Library',
		data: [
			{
				icon: <GearSix weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'LibraryGeneralSettings',
				title: 'General'
			},
			{
				icon: <HardDrive weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'LocationSettings',
				title: 'Locations'
			},
			{
				icon: <ShareNetwork weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'NodesSettings',
				title: 'Nodes'
			},
			{
				icon: <TagSimple weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'TagsSettings',
				title: 'Tags'
			},
			{
				icon: <Key weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'KeysSettings',
				title: 'Keys'
			}
		]
	},
	{
		title: 'Resources',
		data: [
			{
				icon: <FlyingSaucer weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'About',
				title: 'About'
			},
			{
				icon: <Heart weight="bold" color={tw.color('ink')} size={18} />,
				navigateTo: 'Support',
				title: 'Support'
			}
		]
	}
];

function SettingsItem(props: SettingsItemType) {
	const navigation = useNavigation<SettingsStackScreenProps<'Home'>['navigation']>();

	return (
		<Pressable onPress={() => navigation.navigate(props.navigateTo)}>
			<View style={tw`flex flex-row items-center px-2 py-3 bg-app-highlight/40 rounded mb-1.5`}>
				{props.icon}
				<Text style={tw`text-ink ml-2`}>{props.title}</Text>
			</View>
		</Pressable>
	);
}

function renderSectionHeader({ section }: { section: { title: string } }) {
	return (
		<Text
			style={tw.style(
				'mb-2 ml-1 text-base font-semibold text-ink-dull',
				section.title === 'Client' ? 'mt-2' : 'mt-5'
			)}
		>
			{section.title}
		</Text>
	);
}

export default function SettingsScreen({ navigation }: SettingsStackScreenProps<'Home'>) {
	return (
		<View style={tw`flex-1`}>
			<SectionList
				sections={sections}
				contentContainerStyle={tw`p-4`}
				renderItem={({ item }) => (
					<SettingsItem icon={item.icon} title={item.title} navigateTo={item.navigateTo} />
				)}
				renderSectionHeader={renderSectionHeader}
				ListFooterComponent={
					<View style={tw`items-center mb-4 mt-6`}>
						<Text style={tw`text-sm font-bold text-ink`}>Spacedrive</Text>
						<Text style={tw`text-ink-dull text-xs mt-0.5`}>v0.1.0</Text>
					</View>
				}
				showsVerticalScrollIndicator={false}
				stickySectionHeadersEnabled={false}
				initialNumToRender={50}
			/>
		</View>
	);
}
