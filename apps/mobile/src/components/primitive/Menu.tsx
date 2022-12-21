import { Icon } from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { MenuOption, MenuOptions, MenuTrigger, Menu as PMenu } from 'react-native-popup-menu';
import tw from '~/lib/tailwind';

type MenuProps = {
	trigger: React.ReactNode;
	children: React.ReactNode[] | React.ReactNode;
};

export const Menu = (props: MenuProps) => (
	<View>
		<PMenu>
			<MenuTrigger>{props.trigger}</MenuTrigger>
			<MenuOptions>{props.children}</MenuOptions>
		</PMenu>
	</View>
);

type MenuItemProps = {
	text: string;
	icon?: Icon;
	onSelect?: () => void;
};

export const MenuItem = (props: MenuItemProps) => {
	return (
		<>
			<MenuOption style={tw`flex flex-row items-center`} onSelect={props.onSelect}>
				{props.icon && props.icon({ size: 18 })}
				<Text style={tw`ml-2`}>{props.text}</Text>
			</MenuOption>
		</>
	);
};
