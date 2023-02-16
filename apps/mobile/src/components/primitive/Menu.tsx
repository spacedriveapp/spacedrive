import { Icon } from 'phosphor-react-native';
import { View } from 'react-native';
import {
	MenuOption,
	MenuOptionProps,
	MenuOptions,
	MenuTrigger,
	Menu as PMenu,
	renderers
} from 'react-native-popup-menu';
import tw from '~/lib/tailwind';

type MenuProps = {
	trigger: React.ReactNode;
	children: React.ReactNode[] | React.ReactNode;
};

// TODO: Still looks a bit off...
export const Menu = (props: MenuProps) => (
	<View>
		<PMenu renderer={renderers.NotAnimatedContextMenu}>
			<MenuTrigger>{props.trigger}</MenuTrigger>
			<MenuOptions optionsContainerStyle={tw`bg-app-menu rounded p-1`}>
				{props.children}
			</MenuOptions>
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
			{Icon && (
				<View style={tw`ml-1`}>
					<Icon />
				</View>
			)}
			<MenuOption
				{...props}
				customStyles={{
					optionText: tw`text-ink py-0.5 text-sm font-medium`
				}}
				style={tw`flex flex-row items-center`}
			/>
		</View>
	);
};
