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
			<View style={tw`bg-app-overlay flex flex-row items-center justify-between px-3`}>
				<View style={tw`flex flex-row items-center py-4`}>
					{props.leftIcon && props.leftIcon({ size: 18, color: tw.color('ink'), style: tw`mr-2` })}
					<Text style={tw`text-ink text-sm`}>{props.title}</Text>
				</View>
				{props.rightArea ? props.rightArea : <CaretRight size={20} color={tw.color('ink-faint')} />}
			</View>
		</Pressable>
	);
}

export function SettingsItemDivider() {
	return (
		<View style={tw`bg-app-overlay`}>
			<View style={tw`border-b-app-line mx-3 border-b`} />
		</View>
	);
}
