import { CaretRight, Icon } from 'phosphor-react-native';
import { Pressable, Text, View, ViewStyle } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

type SettingsItemProps = {
	title: string;
	onPress?: () => void;
	leftIcon?: Icon;
	rightArea?: React.ReactNode;
};

export function SettingsItem(props: SettingsItemProps) {
	return (
		<Pressable onPress={props.onPress}>
			<View style={tw`flex flex-row items-center justify-between bg-app-box px-4`}>
				<View style={tw`flex flex-row items-center py-3`}>
					{props.leftIcon &&
						props.leftIcon({ size: 20, color: tw.color('ink'), style: tw`mr-3` })}
					<Text style={tw`text-[14px] font-medium text-ink`}>{props.title}</Text>
				</View>
				{props.rightArea ? (
					props.rightArea
				) : (
					<CaretRight size={20} color={tw.color('ink')} />
				)}
			</View>
		</Pressable>
	);
}

export function SettingsItemDivider(props: { style?: ViewStyle }) {
	return (
		<View style={twStyle('bg-app-overlay', props.style)}>
			<View style={tw`mx-3 border-b border-b-app-line`} />
		</View>
	);
}
