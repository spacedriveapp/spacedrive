import { useNavigation } from '@react-navigation/native';
import {
	ArchiveBox,
	Briefcase,
	Clock,
	DotsThree,
	Heart,
	Images,
	MapPin,
	UserFocus
} from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { BrowseStackScreenProps } from '~/navigation/tabs/BrowseStack';

import { Button } from '../primitive/Button';
import LibraryItem from './LibraryItem';

const iconStyle = tw`text-ink-faint`;
const iconSize = 24;
export const CATEGORIES_LIST = [
	{ name: 'Albums', icon: <Images size={iconSize} style={iconStyle} /> },
	{ name: 'Places', icon: <MapPin size={iconSize} style={iconStyle} /> },
	{ name: 'People', icon: <UserFocus size={iconSize} style={iconStyle} /> },
	{ name: 'Projects', icon: <Briefcase size={iconSize} style={iconStyle} /> },
	{ name: 'Favorites', icon: <Heart size={iconSize} style={iconStyle} /> },
	{ name: 'Recents', icon: <Clock size={iconSize} style={iconStyle} /> },
	// { name: 'Labels', icon: <Tag size={iconSize} style={iconStyle} /> },
	{ name: 'Imports', icon: <ArchiveBox size={iconSize} style={iconStyle} /> }
];
const BrowseCategories = () => {
	const navigation = useNavigation<BrowseStackScreenProps<'Browse'>['navigation']>();
	return (
		<View style={tw`gap-5 px-5`}>
			<View style={tw`flex-row items-center justify-between`}>
				<Text style={tw`text-lg font-bold text-white`}>Library</Text>
				<Button
					onPress={() => {
						navigation.navigate('Library');
					}}
					style={tw`h-8 w-8 rounded-full`}
					variant="gray"
				>
					<DotsThree weight="bold" size={18} color={'white'} />
				</Button>
			</View>
			<View style={tw`flex-row flex-wrap gap-2`}>
				{CATEGORIES_LIST.slice(0, 4).map((c) => {
					return <LibraryItem key={c.name} icon={c.icon} name={c.name} />;
				})}
			</View>
		</View>
	);
};

export default BrowseCategories;
