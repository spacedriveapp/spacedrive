import {
	Books,
	FlyingSaucer,
	Gear,
	GearSix,
	HardDrive,
	Heart,
	Icon,
	PaintBrush,
	PuzzlePiece,
	ShareNetwork,
	ShieldCheck,
	TagSimple
} from 'phosphor-react-native';
import React from 'react';
import { SectionList, Text, TouchableWithoutFeedback, View } from 'react-native';
import { DebugState, useDebugState, useDebugStateEnabler } from '@sd/client';
import { SettingsItem, SettingsItemDivider } from '~/components/settings/SettingsItem';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackParamList, SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

type SectionType = {
	title: string;
	data: {
		title: string;
		icon: Icon;
		navigateTo: keyof SettingsStackParamList;
	}[];
};

const sections: (debugState: DebugState) => SectionType[] = (debugState) => [
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
			}
			// {
			// 	icon: Key,
			// 	navigateTo: 'KeysSettings',
			// 	title: 'Keys'
			// }
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
			},
			...(debugState.enabled
				? ([
						{
							icon: Gear,
							navigateTo: 'Debug',
							title: 'Debug'
						}
				  ] as const)
				: [])
		]
	}
];

function renderSectionHeader({ section }: { section: { title: string } }) {
	return (
		<Text
			style={twStyle(
				'mb-2 ml-2 text-sm font-bold text-ink',
				section.title === 'Client' ? 'mt-2' : 'mt-5'
			)}
		>
			{section.title}
		</Text>
	);
}

export default function SettingsScreen({ navigation }: SettingsStackScreenProps<'Home'>) {
	const debugState = useDebugState();

	return (
		<View style={tw`flex-1`}>
			<SectionList
				sections={sections(debugState)}
				contentContainerStyle={tw`py-4`}
				ItemSeparatorComponent={SettingsItemDivider}
				renderItem={({ item }) => (
					<SettingsItem
						title={item.title}
						leftIcon={item.icon}
						onPress={() => navigation.navigate(item.navigateTo as any)}
					/>
				)}
				renderSectionHeader={renderSectionHeader}
				ListFooterComponent={<FooterComponent />}
				showsVerticalScrollIndicator={false}
				stickySectionHeadersEnabled={false}
				initialNumToRender={50}
			/>
		</View>
	);
}

function FooterComponent() {
	const onClick = useDebugStateEnabler();

	return (
		<View style={tw`mb-4 mt-6 items-center`}>
			<TouchableWithoutFeedback onPress={onClick}>
				<Text style={tw`text-base font-bold text-ink`}>Spacedrive</Text>
			</TouchableWithoutFeedback>
			{/* TODO: Get this automatically (expo-device have this?) */}
			<Text style={tw`mt-0.5 text-xs font-medium text-ink-faint`}>v0.1.0</Text>
		</View>
	);
}
