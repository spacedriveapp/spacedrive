import { CaretRight, Icon } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import tw from '~/lib/tailwind';

type SettingsItemProps = {
	title: string;
	onPress?: () => void;
	leftIcon?: Icon;
	rightArea?: React.ReactNode;
};

export function SettingsItem(props: SettingsItemProps) {
	return (
		<Pressable onPress={props.onPress}>
			<View style={tw`flex flex-row items-center justify-between bg-app-overlay px-3`}>
				<View style={tw`flex flex-row items-center py-4`}>
					{props.leftIcon && props.leftIcon({ size: 18, color: tw.color('ink'), style: tw`mr-2` })}
					<Text style={tw`text-sm text-ink`}>{props.title}</Text>
				</View>
				{props.rightArea ? props.rightArea : <CaretRight size={20} color={tw.color('ink-faint')} />}
			</View>
		</Pressable>
	);
}

export function SettingsItemDivider() {
	return (
		<View style={tw`bg-app-overlay`}>
			<View style={tw`mx-3 border-b border-b-app-line`} />
		</View>
	);
}
