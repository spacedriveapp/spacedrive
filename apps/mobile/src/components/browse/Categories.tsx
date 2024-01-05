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
const Categories = () => {
	return (
		<View style={tw`relative gap-5`}>
			<Text style={tw`px-7 text-xl font-bold text-white`}>Library</Text>
			<Fade width={30} height="100%" color="mobile-screen">
				<ScrollView showsHorizontalScrollIndicator={false} horizontal>
					<View style={tw`flex-row gap-2 px-7`}>
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
		<View
			style={tw`h-[70px] w-[70px] flex-col items-center justify-center rounded-md border border-sidebar-line/50 bg-sidebar-box`}
		>
			{icon}
			<Text style={tw`mt-2 text-[12px] text-white`}>{name}</Text>
		</View>
	);
};

export default Categories;
