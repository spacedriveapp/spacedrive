import { Icon } from 'phosphor-react-native';
import { View } from 'react-native';
import {
	MenuOption,
	MenuOptionProps,
	MenuOptions,
	MenuTrigger,
	Menu as PMenu
} from 'react-native-popup-menu';
import { ClassInput } from 'twrnc';
import { tw, twStyle } from '~/lib/tailwind';

type MenuProps = {
	trigger: React.ReactNode;
	children: React.ReactNode[] | React.ReactNode;
	triggerStyle?: ClassInput;
	containerStyle?: ClassInput;
};

// TODO: Still looks a bit off...
export const Menu = (props: MenuProps) => (
	<PMenu style={twStyle(props.triggerStyle)}>
		<MenuTrigger>{props.trigger}</MenuTrigger>
		<MenuOptions
			optionsContainerStyle={twStyle(
				`rounded-md border border-app-cardborder bg-app-menu p-1`,
				props.containerStyle
			)}
		>
			{props.children}
		</MenuOptions>
	</PMenu>
);

type MenuItemProps = {
	icon?: Icon;
	textStyle?: ClassInput;
	iconStyle?: ClassInput;
	style?: ClassInput;
} & MenuOptionProps;

export const MenuItem = ({ icon, textStyle, iconStyle, style, ...props }: MenuItemProps) => {
	const Icon = icon;

	return (
		<View style={twStyle(`flex-1 flex-row items-center px-2 py-1`, style)}>
			{Icon && <Icon size={14} style={twStyle(`text-ink-dull`, iconStyle)} />}
			<MenuOption
				{...props}
				customStyles={{
					optionText: twStyle(`text-sm font-medium text-ink-dull`, textStyle)
				}}
				style={tw`flex flex-row`}
			/>
		</View>
	);
};
