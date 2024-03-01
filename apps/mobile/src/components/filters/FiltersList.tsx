import { AnimatePresence, MotiView } from 'moti';
import {
	CircleDashed,
	Cube,
	Folder,
	IconProps,
	SelectionSlash,
	Textbox
} from 'phosphor-react-native';
import React, { FunctionComponent, useState } from 'react';
import { Pressable, Text, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

import SectionTitle from '../layout/SectionTitle';
import Locations from './Locations';
import Tags from './Tags';

// Extract option names from filters
export type Filters = 'Locations' | 'Tags' | 'Kind' | 'Name' | 'Extension' | 'Hidden';

const FiltersList = () => {
	const [selectedOptions, setSelectedOptions] = useState<Partial<Filters[]>>([]);
	const options = [
		{
			name: 'Locations',
			icon: Folder,
			component: Locations
		},
		{
			name: 'Tags',
			icon: CircleDashed,
			component: Tags
		},
		{ name: 'Kind', icon: Cube, component: () => <></> },
		{ name: 'Name', icon: Textbox, component: () => <></> },
		{ name: 'Extension', icon: Textbox, component: () => <></> },
		{ name: 'Hidden', icon: SelectionSlash, component: () => <></> }
	] as const;

	const selectedHandler = (option: Filters) => {
		setSelectedOptions((p) => {
			if (p.includes(option)) {
				return p.filter((name) => name !== option);
			} else {
				return [...p, option];
			}
		});
	};

	return (
		<View style={tw`gap-10`}>
			<View>
				<SectionTitle
					style={tw`px-6 pb-3`}
					title="What are you searching for?"
					sub="Tap the filter(s) you’d like to use as a query"
				/>
				<View style={tw`flex-row justify-between gap-2 px-6`}>
					{/* 2 column layout */}
					<View style={tw`flex-1 gap-2`}>
						{options.slice(0, options.length / 2).map((option, index) => (
							<MotiView
								from={{ opacity: 0, translateY: 20 }}
								animate={{ opacity: 1, translateY: 0 }}
								transition={{ type: 'timing', duration: 300, delay: index * 100 }}
								key={option.name}
							>
								<FilterOption
									onPress={() => selectedHandler(option.name)}
									isSelected={selectedOptions.includes(option.name)}
									key={index}
									name={option.name}
									Icon={option.icon}
								/>
							</MotiView>
						))}
					</View>
					<View style={tw`flex-1 gap-2`}>
						{options.slice(options.length / 2, options.length).map((option, index) => (
							<MotiView
								from={{ opacity: 0, translateY: 20 }}
								animate={{ opacity: 1, translateY: 0 }}
								transition={{
									type: 'timing',
									duration: 300,
									delay: index * 100 + 200
								}}
								key={option.name}
							>
								<FilterOption
									onPress={() => selectedHandler(option.name)}
									isSelected={selectedOptions.includes(option.name)}
									key={index}
									name={option.name}
									Icon={option.icon}
								/>
							</MotiView>
						))}
					</View>
				</View>
			</View>
			{/* conditionally render the selected options - this approach makes sure the animation is right
			by not relying on the index position of the object */}
			<AnimatePresence>
				{selectedOptions.map((option) => {
					const Component = options.find((o) => o.name === option)?.component;
					if (!Component) return null;
					return <Component key={option} />;
				})}
			</AnimatePresence>
		</View>
	);
};

interface FilterOptionProps {
	name: string;
	Icon: FunctionComponent<IconProps>;
	onPress: () => void;
	isSelected: boolean;
}

const FilterOption = ({ name, Icon, onPress, isSelected }: FilterOptionProps) => {
	return (
		<Pressable onPress={onPress}>
			<MotiView
				animate={{
					borderColor: isSelected ? tw.color('accent') : tw.color('app-line/50')
				}}
				transition={{ type: 'timing', duration: 300 }}
				style={twStyle(
					`w-full flex-row items-center justify-center gap-1 rounded-md border py-2.5`,
					isSelected ? ' bg-app-box' : 'bg-app-box/50'
				)}
			>
				<Icon size={18} color={tw.color(isSelected ? 'ink' : 'ink-dull')} />
				<Text style={tw`text-sm font-medium text-ink`}>{name}</Text>
			</MotiView>
		</Pressable>
	);
};

export default FiltersList;
