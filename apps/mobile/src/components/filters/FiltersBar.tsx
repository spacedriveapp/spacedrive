import {
	CircleDashed,
	Cube,
	Folder,
	IconProps,
	Plus,
	SelectionSlash,
	Textbox,
	X
} from 'phosphor-react-native';
import { Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';
import { SearchFilters, useSearchStore } from '~/stores/searchStore';

import { Button } from '../primitive/Button';

const FiltersBar = () => {
	const searchStore = useSearchStore();
	const filters = searchStore.filters;
	return (
		<View
			style={tw`w-full flex-row items-center gap-4 border-t border-app-line/50 bg-mobile-screen px-5 py-3`}
		>
			<Button style={tw`border-2 p-1.5`} variant="dashed">
				<Plus weight="bold" size={20} color={tw.color('text-ink-dull')} />
			</Button>
			<FilterItem filter="Tags" value={''} />
		</View>
	);
};

interface FilterItemProps {
	filter: Capitalize<SearchFilters>;
	value: any;
}

const FilterItem = ({ filter, value }: FilterItemProps) => {
	const iconStyle = tw`text-ink-dull`;
	const boxStyle = tw`w-auto flex-row items-center gap-1.5 border border-app-line/50 bg-app-box/50 p-2`;

	const FilterIcon = (props: IconProps) => {
		switch (filter) {
			case 'Tags':
				return <CircleDashed {...props} />;
			case 'Kind':
				return <Cube {...props} />;
			case 'Name':
				return <Textbox {...props} />;
			case 'Extension':
				return <Textbox {...props} />;
			case 'Hidden':
				return <SelectionSlash {...props} />;
			default:
				return <Folder {...props} />;
		}
	};

	return (
		<View style={tw`flex-row gap-0.5`}>
			<View style={twStyle(boxStyle, 'rounded-bl-md rounded-tl-md')}>
				<FilterIcon size={18} style={iconStyle} />
				<Text style={tw`text-md text-ink`}>{filter}</Text>
			</View>
			<View style={twStyle(boxStyle, 'rounded-br-md rounded-tr-md')}>
				<X size={18} style={tw`text-ink-dull`} />
			</View>
		</View>
	);
};

export default FiltersBar;
