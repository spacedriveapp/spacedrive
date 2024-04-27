import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { RouteProp, useNavigation } from '@react-navigation/native';
import { NativeStackHeaderProps } from '@react-navigation/native-stack';
import { ArrowLeft, DotsThreeOutline, MagnifyingGlass } from 'phosphor-react-native';
import { Platform, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { tw, twStyle } from '~/lib/tailwind';
import { getExplorerStore, useExplorerStore } from '~/stores/explorerStore';
import { Icon } from '../icons/Icon';


type Props = {
	headerRoute?: NativeStackHeaderProps; //supporting title from the options object of navigation
	optionsRoute?: RouteProp<any, any>; //supporting params passed
	kind: 'tag' | 'location'; //the kind of icon to display
	explorerMenu?: boolean; //whether to show the explorer menu
};

export default function DynamicHeader({
	headerRoute,
	optionsRoute,
	kind,
	explorerMenu = true
}: Props) {
	const navigation = useNavigation<DrawerNavigationHelpers>();
	const headerHeight = useSafeAreaInsets().top;
	const isAndroid = Platform.OS === 'android';
	const explorerStore = useExplorerStore();

	return (
		<View
			style={twStyle('relative h-auto w-full border-b border-app-cardborder bg-app-header', {
				paddingTop: headerHeight + (isAndroid ? 15 : 0)
			})}
		>
			<View style={tw`mx-auto h-auto w-full justify-center px-5 pb-3`}>
				<View style={tw`w-full flex-row items-center justify-between`}>
					<View style={tw`flex-row items-center gap-3`}>
					<Pressable
								hitSlop={24}
								onPress={() => navigation.goBack()}
							>
								<ArrowLeft size={23} color={tw.color('ink')} />
							</Pressable>
						<View style={tw`flex-row items-center gap-1.5`}>
						<HeaderIconKind routeParams={optionsRoute?.params} kind={kind} />
						<Text
								numberOfLines={1}
								style={tw`max-w-[200px] text-xl font-bold text-white`}
							>
								{headerRoute?.options.title}
							</Text>
						</View>
					</View>
				<View style={tw`flex-row gap-3`}>
				{explorerMenu && <Pressable
								hitSlop={12}
								onPress={() => {
									getExplorerStore().toggleMenu = !explorerStore.toggleMenu;
								}}
							>
								<DotsThreeOutline
									size={24}
									color={tw.color(
										explorerStore.toggleMenu ? 'text-accent' : 'text-zinc-300'
									)}
								/>
							</Pressable>}
						<Pressable
									hitSlop={12}
									onPress={() => {
										navigation.navigate('SearchStack', {
											screen: 'Search'
										});
									}}
								>
									<MagnifyingGlass
										size={24}
										weight="bold"
										color={tw.color('text-zinc-300')}
									/>
								</Pressable>
				</View>
				</View>
			</View>
		</View>
	);
}

interface HeaderIconKindProps {
	routeParams?: any;
	kind: Props['kind'];
}

const HeaderIconKind = ({routeParams, kind }: HeaderIconKindProps) => {
	switch (kind) {
		case 'location':
			return <Icon size={30} name="Folder" />;
		case 'tag':
			return (
				<View
					style={twStyle('h-[24px] w-[24px] rounded-full', {
						backgroundColor: routeParams.color
					})}
				/>
			);
		default:
			return null;
	}
};
