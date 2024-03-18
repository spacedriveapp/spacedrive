import {
	ArchiveBox,
	Briefcase,
	Clock,
	Heart,
	Images,
	MapPin,
	Tag,
	UserFocus
} from 'phosphor-react-native';
import { ReactElement } from 'react';
import { Text, View } from 'react-native';
import { ScrollView } from 'react-native-gesture-handler';
import { tw } from '~/lib/tailwind';

import Fade from '../layout/Fade';
import Card from '../layout/Card';

const iconStyle = tw`text-[17px] text-ink-dull`;
const CATEGORIES_LIST = [
	{ name: 'Albums', icon: <Images style={iconStyle} /> },
	{ name: 'Places', icon: <MapPin style={iconStyle} /> },
	{ name: 'People', icon: <UserFocus style={iconStyle} /> },
	{ name: 'Projects', icon: <Briefcase style={iconStyle} /> },
	{ name: 'Favorites', icon: <Heart style={iconStyle} /> },
	{ name: 'Recents', icon: <Clock style={iconStyle} /> },
	{ name: 'Labels', icon: <Tag style={iconStyle} /> },
	{ name: 'Imports', icon: <ArchiveBox style={iconStyle} /> }
];
const BrowseCategories = () => {
	return (
		<View style={tw`relative gap-3`}>
			<Text style={tw`px-6 text-lg font-bold text-white`}>Library</Text>
			<Fade width={30} height="100%" color="black">
				<ScrollView showsHorizontalScrollIndicator={false} horizontal>
					<View style={tw`flex-row gap-2 px-6`}>
						{CATEGORIES_LIST.map((c, i) => {
							return <Category icon={c.icon} key={i} name={c.name} />;
						})}
					</View>
				</ScrollView>
			</Fade>
		</View>
	);
};

interface CategoryProps {
	name: string;
	icon: ReactElement;
}

const Category = ({ name, icon }: CategoryProps) => {
	return (
		<Card style="h-[70px] w-[70px] items-center justify-center">
			{icon}
			<Text style={tw`mt-2 text-xs text-white`}>{name}</Text>
		</Card>
	);
};

export default BrowseCategories;
