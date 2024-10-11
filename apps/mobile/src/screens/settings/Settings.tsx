import {
	ArrowsClockwise,
	Books,
	Cloud,
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
	TagSimple,
	UserCircle
} from 'phosphor-react-native';
import { Platform, SectionList, Text, TouchableWithoutFeedback, View } from 'react-native';
import { DebugState, useDebugState, useDebugStateEnabler, useLibraryQuery } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { SettingsItem } from '~/components/settings/SettingsItem';
import { useEnableDrawer } from '~/hooks/useEnableDrawer';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackParamList, SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';
import { useUserStore } from '~/stores/userStore';

type SectionType = {
	title: string;
	data: {
		title: string;
		icon: Icon;
		navigateTo: keyof SettingsStackParamList;
		comingSoon?: boolean;
		rounded?: 'top' | 'bottom';
	}[];
};

const sections: (
	debugState: DebugState,
	userInfo: ReturnType<typeof useUserStore>['userInfo']
) => SectionType[] = (debugState, userInfo) => [
	{
		title: 'Client',
		data: [
			{
				icon: GearSix,
				navigateTo: 'GeneralSettings',
				title: 'General',
				rounded: 'top'
			},
			...(userInfo
				? ([
						{
							icon: UserCircle,
							navigateTo: 'AccountProfile',
							title: 'Account'
						}
					] as const)
				: ([
						{
							icon: UserCircle,
							navigateTo: 'AccountLogin',
							title: 'Account'
						}
					] as const)),
			{
				icon: Books,
				navigateTo: 'LibrarySettings',
				title: 'Libraries'
			},
			{
				icon: PaintBrush,
				comingSoon: true,
				navigateTo: 'AppearanceSettings',
				title: 'Appearance'
			},
			{
				icon: ShieldCheck,
				navigateTo: 'PrivacySettings',
				comingSoon: true,
				title: 'Privacy'
			},
			{
				icon: PuzzlePiece,
				navigateTo: 'ExtensionsSettings',
				title: 'Extensions',
				rounded: 'bottom',
				comingSoon: true
			}
		]
	},
	{
		title: 'Library',
		data: [
			{
				icon: GearSix,
				navigateTo: 'LibraryGeneralSettings',
				title: 'General',
				rounded: 'top'
			},
			{
				icon: HardDrive,
				navigateTo: 'LocationSettings',
				title: 'Locations'
			},
			{
				icon: ShareNetwork,
				navigateTo: 'NodesSettings',
				comingSoon: true,
				title: 'Nodes'
			},
			{
				icon: TagSimple,
				navigateTo: 'TagsSettings',
				title: 'Tags'
			},
			{
				icon: Cloud,
				navigateTo: 'CloudSettings',
				title: 'Cloud'
			},
			{
				icon: ArrowsClockwise,
				navigateTo: 'SyncSettings',
				title: 'Sync',
				rounded: 'bottom'
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
				title: 'About',
				rounded: 'top'
			},
			{
				icon: Heart,
				navigateTo: 'Support',
				title: 'Support',
				rounded: !debugState.enabled ? 'bottom' : undefined
			},
			...(debugState.enabled
				? ([
						{
							icon: Gear,
							navigateTo: 'Debug',
							title: 'Debug',
							rounded: 'bottom'
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
				'mb-3 text-lg font-bold text-ink',
				section.title === 'Client' ? 'mt-0' : 'mt-5'
			)}
		>
			{section.title}
		</Text>
	);
}

export default function SettingsScreen({ navigation }: SettingsStackScreenProps<'Settings'>) {
	const debugState = useDebugState();
	const syncEnabled = useLibraryQuery(['sync.enabled']);
	const userInfo = useUserStore().userInfo;

	// Enables the drawer from react-navigation
	useEnableDrawer();

	return (
		<ScreenContainer tabHeight={false} style={tw`gap-0 px-5 py-0`}>
			<SectionList
				contentContainerStyle={tw`py-6`}
				sections={sections(debugState, userInfo)}
				renderItem={({ item }) => (
					<SettingsItem
						syncEnabled={syncEnabled.data}
						comingSoon={item.comingSoon}
						title={item.title}
						leftIcon={item.icon}
						onPress={() => navigation.navigate(item.navigateTo as any)}
						rounded={item.rounded}
					/>
				)}
				scrollEnabled={false}
				renderSectionHeader={renderSectionHeader}
				ListFooterComponent={<FooterComponent />}
				showsVerticalScrollIndicator={false}
				stickySectionHeadersEnabled={false}
				initialNumToRender={50}
			/>
		</ScreenContainer>
	);
}

function FooterComponent() {
	const onClick = useDebugStateEnabler();
	return (
		<View
			style={twStyle(Platform.OS === 'android' ? 'mb-14 mt-4' : 'mb-20 mt-5', 'items-center')}
		>
			<TouchableWithoutFeedback onPress={onClick}>
				<Text style={tw`text-base font-bold text-ink`}>Spacedrive</Text>
			</TouchableWithoutFeedback>
			{/* TODO: Get this automatically (expo-device have this?) */}
			<Text style={tw`mt-0.5 text-xs font-medium text-ink-faint`}>v0.1.0</Text>
		</View>
	);
}
