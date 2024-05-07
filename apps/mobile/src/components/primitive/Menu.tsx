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
import { ClassInput } from 'twrnc';
import { tw, twStyle } from '~/lib/tailwind';

type MenuProps = {
	trigger: React.ReactNode;
	children: React.ReactNode[] | React.ReactNode;
	triggerStyle?: ClassInput;
};

// TODO: Still looks a bit off...
export const Menu = (props: MenuProps) => (
		<PMenu renderer={renderers.NotAnimatedContextMenu} style={twStyle(props.triggerStyle)}>
			<MenuTrigger>{props.trigger}</MenuTrigger>
			<MenuOptions optionsContainerStyle={tw`rounded-md border border-app-cardborder bg-app-menu p-1`}>
				{props.children}
			</MenuOptions>
		</PMenu>
);

type MenuItemProps = {
	icon?: Icon;
} & MenuOptionProps;

export const MenuItem = ({ icon, ...props }: MenuItemProps) => {
	const Icon = icon;

	return (
		<View style={tw`flex flex-1 flex-row items-center`}>
			{Icon && (
				<View style={tw`ml-1`}>
					<Icon size={16} style={tw`text-ink`} />
				</View>
			)}
			<MenuOption
				{...props}
				customStyles={{
					optionText: tw`w-full py-1 text-sm font-medium text-ink`
				}}
				style={tw`flex flex-row items-center`}
			/>
		</View>
	);
};
