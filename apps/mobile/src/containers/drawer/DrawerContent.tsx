import { DrawerContentScrollView } from '@react-navigation/drawer';
import { DrawerContentComponentProps } from '@react-navigation/drawer/lib/typescript/src/types';
import { Gear } from 'phosphor-react-native';
import { Image, Platform, Pressable, Text, View } from 'react-native';
import Layout from '~/constants/Layout';
import tw from '~/lib/tailwind';
import { getStackNameFromState } from '~/utils/nav';
import Divider from '../../components/primitive/Divider';
import DrawerLibraryManager from './DrawerLibraryManager';
import DrawerLocations from './DrawerLocations';
import DrawerTags from './DrawerTags';

const drawerHeight = Platform.select({
	ios: Layout.window.height * 0.85,
	android: Layout.window.height * 0.9
});

const DrawerContent = ({ navigation, state }: DrawerContentComponentProps) => {
	const stackName = getStackNameFromState(state);

	return (
		<DrawerContentScrollView style={tw`flex-1 px-4 py-2`} scrollEnabled={false}>
			<View style={tw.style('justify-between', { height: drawerHeight })}>
				<View>
					<View style={tw`flex flex-row items-center`}>
						<Image source={require('@sd/assets/images/logo.png')} style={tw`h-[35px] w-[35px]`} />
						<Text style={tw`ml-2 text-base font-bold text-ink`}>Spacedrive</Text>
					</View>
					<Divider style={tw`my-4`} />
					{/* Library Manager */}
					<DrawerLibraryManager />
					{/* Locations */}
					<DrawerLocations stackName={stackName} />
					{/* Tags */}
					<DrawerTags stackName={stackName} />
				</View>
				{/* Settings */}
				<Pressable onPress={() => navigation.navigate('Settings')}>
					<Gear color={tw.color('ink')} size={24} />
				</Pressable>
			</View>
		</DrawerContentScrollView>
	);
};

export default DrawerContent;
