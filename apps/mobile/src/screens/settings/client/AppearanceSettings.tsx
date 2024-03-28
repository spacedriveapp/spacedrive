import { CheckCircle } from 'phosphor-react-native';
import React, { useState } from 'react';
import { ColorValue, Pressable, ScrollView, Text, View, ViewStyle } from 'react-native';
import { Themes, useThemeStore } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import Colors from '~/constants/style/Colors';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

type Theme = {
	insideColor: ColorValue;
	outsideColor: ColorValue;
	textColor: ColorValue;
	highlightColor: ColorValue;
	themeName: string;
	themeValue: Themes | 'system';
};

// TODO: Once theming is fixed, use theme values for Light theme too.
const themes: Theme[] = [
	{
		insideColor: Colors.vanilla.app.DEFAULT,
		outsideColor: '#F0F0F0',
		textColor: Colors.vanilla.ink.DEFAULT,
		highlightColor: '#E6E6E6',
		themeName: 'Light',
		themeValue: 'vanilla'
	},
	{
		insideColor: Colors.dark.app.DEFAULT,
		outsideColor: Colors.dark.app.darkBox,
		textColor: Colors.dark.ink.DEFAULT,
		highlightColor: Colors.dark.app.line,
		themeName: 'Dark',
		themeValue: 'dark'
	},
	{
		insideColor: '#000000',
		outsideColor: '#000000',
		textColor: '#000000',
		highlightColor: '#000000',
		themeName: 'System',
		themeValue: 'system'
	}
];

type ThemeProps = Theme & { isSelected?: boolean; containerStyle?: ViewStyle };

function Theme(props: ThemeProps) {
	return (
		<View style={twStyle(props.containerStyle)}>
			<View
				style={twStyle(
					{ backgroundColor: props.outsideColor },
					'relative h-[80px] w-[100px] overflow-hidden rounded-xl'
				)}
			>
				<View
					style={twStyle(
						{ backgroundColor: props.insideColor, borderColor: props.highlightColor },
						'absolute -bottom-px -right-px h-[60px] w-[75px] rounded-tl-xl border'
					)}
				>
					<Text
						style={twStyle({ color: props.textColor }, 'ml-3 mt-1 text-lg font-medium')}
					>
						Aa
					</Text>
				</View>
				{/* Checkmark */}
				{props.isSelected && (
					<CheckCircle
						color={tw.color('accent')}
						weight="fill"
						size={24}
						style={tw`absolute bottom-1.5 right-1.5`}
					/>
				)}
			</View>
		</View>
	);
}

function SystemTheme(props: { isSelected: boolean }) {
	return (
		<View style={tw`h-[80px] w-[110px] flex-row overflow-hidden rounded-xl`}>
			<View
				style={twStyle('flex-1 overflow-hidden', {
					backgroundColor: themes[1]!.outsideColor
				})}
			>
				<View style={tw`absolute`}>
					<Theme {...themes[1]!} containerStyle={tw`right-3`} />
				</View>
			</View>
			<View
				style={twStyle('flex-1 overflow-hidden', {
					backgroundColor: themes[0]!.outsideColor
				})}
			>
				<Theme {...themes[0]!} containerStyle={tw`right-3`} />
			</View>
			{/* Checkmark */}
			{props.isSelected && (
				<CheckCircle
					color={tw.color('accent')}
					weight="fill"
					size={24}
					style={tw`absolute bottom-1.5 right-1.5`}
				/>
			)}
		</View>
	);
}

const AppearanceSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'AppearanceSettings'>) => {
	const themeStore = useThemeStore();

	const [selectedTheme, setSelectedTheme] = useState<Theme['themeValue']>(
		themeStore.syncThemeWithSystem === true ? 'system' : themeStore.theme
	);

	// TODO: Hook this up to the theme store once light theme is fixed.

	return (
		<ScreenContainer scrollview={false} style={tw`gap-2 px-6`}>
			<SettingsTitle>Theme</SettingsTitle>
			<ScrollView
				horizontal
				showsHorizontalScrollIndicator={false}
				contentContainerStyle={tw`gap-x-3`}
			>
				{themes.map((theme) => (
					<Pressable
						key={theme.themeValue}
						onPress={() => setSelectedTheme(theme.themeValue)}
					>
						{theme.themeValue === 'system' ? (
							<SystemTheme isSelected={selectedTheme === 'system'} />
						) : (
							<Theme {...theme} isSelected={selectedTheme === theme.themeValue} />
						)}
						<Text style={tw`mt-1.5 text-center font-medium text-white`}>
							{theme.themeName}
						</Text>
					</Pressable>
				))}
			</ScrollView>
		</ScreenContainer>
	);
};

export default AppearanceSettingsScreen;
