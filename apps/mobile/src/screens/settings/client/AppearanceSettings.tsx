import { CheckCircle } from 'phosphor-react-native';
import React, { useState } from 'react';
import { ColorValue, Pressable, ScrollView, Text, View, ViewStyle } from 'react-native';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw, twStyle } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

type Themes = {
	insideColor: ColorValue;
	outsideColor: ColorValue;
	textColor: ColorValue;
	name: string;
};

const themes: Themes[] = [
	{
		insideColor: '#FFFFFF',
		outsideColor: '#000000',
		textColor: '#000000',
		name: 'Light'
	},
	{
		insideColor: '#000000',
		outsideColor: '#FFFFFF',
		textColor: '#FFFFFF',
		name: 'Dark'
	},
	{
		insideColor: '#000000',
		outsideColor: '#000000',
		textColor: '#000000',
		name: 'System'
	}
];

type ThemeProps = Themes & { isSelected?: boolean; containerStyle?: ViewStyle };

function Theme(props: ThemeProps) {
	return (
		<View style={props.containerStyle}>
			<View
				style={twStyle(
					{ backgroundColor: props.outsideColor },
					'relative h-[90px] w-[110px] overflow-hidden rounded-xl border border-transparent',
					props.isSelected && { borderColor: props.insideColor }
				)}
			>
				<View
					style={twStyle(
						{ backgroundColor: props.insideColor },
						'absolute bottom-[-1px] right-[-1px] h-[65px] w-[80px] rounded-tl-xl'
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
						color={props.outsideColor as string}
						weight="fill"
						size={24}
						style={tw`absolute right-2 bottom-2`}
					/>
				)}
			</View>
		</View>
	);
}

// TODO: WIP
function SystemTheme(props: { isSelected: boolean }) {
	return (
		<View style={tw`h-[90px] w-[110px] flex-1 flex-row overflow-hidden rounded-xl`}>
			<View style={tw`z-10 flex-1`}>
				<Theme {...themes[1]!} containerStyle={tw`absolute top-0 left-10 z-10`} />
			</View>
			<View style={tw`flex-1 bg-red-200`}>
				<Theme {...themes[0]!} containerStyle={tw`bottom-0 right-6`} />
			</View>
		</View>
	);
}

const AppearanceSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'AppearanceSettings'>) => {
	const [selectedTheme, setSelectedTheme] = useState(themes[2]?.name);
	return (
		<View style={tw`flex-1 pt-4`}>
			<View style={tw`px-4`}>
				<SettingsTitle>Theme</SettingsTitle>
				<ScrollView
					horizontal
					showsHorizontalScrollIndicator={false}
					contentContainerStyle={tw`gap-x-2`}
				>
					{themes.map((theme) => (
						<Pressable key={theme.name} onPress={() => setSelectedTheme(theme.name)}>
							{theme.name === 'System' ? (
								<SystemTheme isSelected={selectedTheme === 'System'} />
							) : (
								<Theme {...theme} isSelected={selectedTheme === theme.name} />
							)}
							<Text style={tw`mt-1.5 text-center font-medium text-white`}>
								{theme.name}
							</Text>
						</Pressable>
					))}
				</ScrollView>
			</View>
		</View>
	);
};

export default AppearanceSettingsScreen;
