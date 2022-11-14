import { useNavigation } from '@react-navigation/native';
import {
	Books,
	FlyingSaucer,
	GearSix,
	HardDrive,
	Heart,
	Icon,
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
	icon: Icon;
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
				icon: GearSix,
				navigateTo: 'GeneralSettings',
				title: 'General'
			},
			{
				icon: Books,
				navigateTo: 'LibrarySettings',
				title: 'Libraries'
			},
			{
				icon: PaintBrush,
				navigateTo: 'AppearanceSettings',
				title: 'Appearance'
			},
			{
				icon: ShieldCheck,
				navigateTo: 'PrivacySettings',
				title: 'Privacy'
			},
			{
				icon: PuzzlePiece,
				navigateTo: 'ExtensionsSettings',
				title: 'Extensions'
			}
		]
	},
	{
		title: 'Library',
		data: [
			{
				icon: GearSix,
				navigateTo: 'LibraryGeneralSettings',
				title: 'General'
			},
			{
				icon: HardDrive,
				navigateTo: 'LocationSettings',
				title: 'Locations'
			},
			{
				icon: ShareNetwork,
				navigateTo: 'NodesSettings',
				title: 'Nodes'
			},
			{
				icon: TagSimple,
				navigateTo: 'TagsSettings',
				title: 'Tags'
			},
			{
				icon: Key,
				navigateTo: 'KeysSettings',
				title: 'Keys'
			}
		]
	},
	{
		title: 'Resources',
		data: [
			{
				icon: FlyingSaucer,
				navigateTo: 'About',
				title: 'About'
			},
			{
				icon: Heart,
				navigateTo: 'Support',
				title: 'Support'
			}
		]
	}
];

function SettingsItem(props: SettingsItemType) {
	const navigation = useNavigation<SettingsStackScreenProps<'Home'>['navigation']>();

	const Icon = props.icon;

	return (
		<Pressable onPress={() => navigation.navigate(props.navigateTo)}>
			<View style={tw`flex flex-row items-center px-2 py-[10px] bg-app-overlay rounded mb-1.5`}>
				<Icon weight="bold" color={tw.color('ink')} size={18} style={tw`ml-1 mr-2`} />
				<Text style={tw`text-ink text-sm`}>{props.title}</Text>
			</View>
		</Pressable>
	);
}

function renderSectionHeader({ section }: { section: { title: string } }) {
	return (
		<Text
			style={tw.style(
				'mb-2 ml-1 text-sm font-semibold text-ink-dull',
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
