import { Icon } from 'phosphor-react-native';
import { View } from 'react-native';
import {
	MenuOption,
	MenuOptionProps,
	MenuOptions,
	MenuTrigger,
	Menu as PMenu
} from 'react-native-popup-menu';
import tw from '~/lib/tailwind';

type MenuProps = {
	trigger: React.ReactNode;
	children: React.ReactNode[] | React.ReactNode;
};

export const Menu = (props: MenuProps) => (
	<View>
		<PMenu>
			<MenuTrigger>{props.trigger}</MenuTrigger>
			<MenuOptions optionsContainerStyle={tw`bg-app-menu`}>{props.children}</MenuOptions>
		</PMenu>
	</View>
);

type MenuItemProps = {
	icon?: Icon;
} & MenuOptionProps;

export const MenuItem = ({ icon, ...props }: MenuItemProps) => {
	const Icon = icon;

	return (
		<View style={tw`flex flex-row items-center`}>
			{Icon && <Icon size={20} color={tw.color('ink-dull')} />}
			<MenuOption
				{...props}
				customStyles={{ optionText: tw`text-ink text-sm` }}
				style={tw`flex flex-row items-center`}
			/>
		</View>
	);
};
