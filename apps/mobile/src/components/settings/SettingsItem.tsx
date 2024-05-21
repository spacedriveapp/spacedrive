import { CaretRight, Icon } from 'phosphor-react-native';
import { Pressable, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

type SettingsItemProps = {
	title: string;
	onPress?: () => void;
	leftIcon?: Icon;
	rightArea?: React.ReactNode;
	rounded?: 'top' | 'bottom';
};

export function SettingsItem(props: SettingsItemProps) {
	//due to SectionList limitation of not being able to modify each section individually
	//we have to use this 'hacky' way to make the top and bottom rounded
	const borderRounded =
		props.rounded === 'top' ? 'rounded-t-md' : props.rounded === 'bottom' && 'rounded-b-md';
	const border =
		props.rounded === 'top'
			? 'border-t border-r border-l'
			: props.rounded === 'bottom'
				? 'border-b border-r border-l'
				: 'border-l border-r';
	return (
		<Pressable onPress={props.onPress}>
			<View style={twStyle(' border-app-cardborder bg-app-card', borderRounded, border)}>
				<View style={tw`h-auto flex-row items-center`}>
					{props.leftIcon && (
						<View
							style={tw`ml-4 mr-5 h-8 w-8 items-center justify-center rounded-full border border-app-lightborder bg-app-button`}
						>
							{props.leftIcon({ size: 20, color: tw.color('ink-dull') })}
						</View>
					)}
					<View
						style={twStyle(
							`flex-1 flex-row items-center justify-between border-b py-4`,
							borderRounded !== 'rounded-b-md'
								? 'border-app-cardborder'
								: 'border-transparent'
						)}
					>
						<Text style={tw`text-sm font-medium text-ink`}>{props.title}</Text>
						<CaretRight style={tw`mr-4`} size={16} color={tw.color('ink')} />
					</View>
				</View>
			</View>
		</Pressable>
	);
}
