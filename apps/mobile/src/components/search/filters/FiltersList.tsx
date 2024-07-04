import { AnimatePresence, MotiView } from 'moti';
import { MotiPressable } from 'moti/interactions';
import {
	CircleDashed,
	Cube,
	Folder,
	IconProps,
	SelectionSlash,
	Textbox
} from 'phosphor-react-native';
import React, { FunctionComponent, useCallback, useEffect, useState } from 'react';
import { Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import SectionTitle from '~/components/layout/SectionTitle';
import { tw, twStyle } from '~/lib/tailwind';
import { getSearchStore, SearchFilters, useSearchStore } from '~/stores/searchStore';

import Extension from './Extension';
import Kind from './Kind';
import Locations from './Locations';
import Name from './Name';
import SavedSearches from './SavedSearches';
import Tags from './Tags';

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
	{ name: 'Kind', icon: Cube, component: Kind },
	{ name: 'Name', icon: Textbox, component: Name },
	{ name: 'Extension', icon: Textbox, component: Extension },
	{
		name: 'Hidden',
		icon: SelectionSlash
	}
] satisfies {
	name: Capitalize<SearchFilters>;
	icon: FunctionComponent<IconProps>;
	component?: FunctionComponent<any>;
}[];

const FiltersList = () => {
	const searchStore = useSearchStore();
	const [selectedOptions, setSelectedOptions] = useState<SearchFilters[]>(
		Object.keys(searchStore.appliedFilters) as SearchFilters[]
	);
	const appliedFiltersLength = Object.keys(searchStore.appliedFilters).length;

	useEffect(() => {
		//if there are selected filters but not applied reset them
		if (appliedFiltersLength === 0) {
			getSearchStore().resetFilters();
		}
	}, [appliedFiltersLength]);

	const selectedHandler = useCallback(
		(option: Capitalize<SearchFilters>) => {
			const searchFiltersLowercase = option.toLowerCase() as SearchFilters; //store values are lowercase
			const isSelected = selectedOptions.includes(searchFiltersLowercase);

			// Since hidden is a boolean - it does not have a component like the other filters
			if (searchFiltersLowercase === 'hidden') {
				searchStore.updateFilters('hidden', !searchStore.filters.hidden);
			}

			// Update selected options
			setSelectedOptions(
				isSelected
					? selectedOptions.filter((o) => o !== searchFiltersLowercase)
					: [...selectedOptions, searchFiltersLowercase]
			);

			// Only reset the filter if it was previously selected
			if (isSelected) {
				searchStore.resetFilter(searchFiltersLowercase);
			}
		},
		[selectedOptions, searchStore]
	);

	return (
		<View style={tw`gap-10`}>
			<SavedSearches />
			<MotiView
				from={{ opacity: 0, translateY: 20 }}
				animate={{ opacity: 1, translateY: 0 }}
				transition={{ type: 'timing', duration: 300 }}
			>
				<SectionTitle
					style={tw`px-6 pb-3`}
					title="What are you searching for?"
					sub="Tap the filters you’d like to use as a query"
				/>
				<View style={tw`flex-row justify-between gap-2 px-6`}>
					{/* 2 column layout */}
					<View style={tw`flex-1 gap-2`}>
						{options.slice(0, options.length / 2).map((option, index) => (
							<MotiPressable
								onPress={() => selectedHandler(option.name)}
								from={{ opacity: 0, translateY: 20 }}
								animate={{ opacity: 1, translateY: 0 }}
								transition={{ type: 'timing', duration: 300, delay: index * 100 }}
								key={option.name}
							>
								<FilterOption
									isSelected={selectedOptions.includes(
										option.name.toLowerCase() as SearchFilters
									)}
									key={index}
									name={option.name}
									Icon={option.icon}
								/>
							</MotiPressable>
						))}
					</View>
					<View style={tw`flex-1 gap-2`}>
						{options.slice(options.length / 2, options.length).map((option, index) => (
							<MotiPressable
								onPress={() => selectedHandler(option.name)}
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
									isSelected={selectedOptions.includes(
										option.name.toLowerCase() as SearchFilters
									)}
									key={index}
									name={option.name}
									Icon={option.icon}
								/>
							</MotiPressable>
						))}
					</View>
				</View>
			</MotiView>
			{/* conditionally render the selected options - this approach makes sure the animation is right
			by not relying on the index position of the object */}
			<AnimatePresence>
				{selectedOptions.map((option) => {
					const capitilize = option.charAt(0).toUpperCase() + option.slice(1);
					const Component = options.find((o) => o.name === capitilize)?.component;
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
	isSelected: boolean;
}

const FilterOption = ({ name, Icon, isSelected }: FilterOptionProps) => {
	return (
		<Card
			style={twStyle(`w-full flex-row items-center justify-center gap-1.5 py-2.5`, {
				borderColor: isSelected ? tw.color('accent') : tw.color('app-cardborder')
			})}
		>
			<Icon size={18} color={tw.color('ink-dull')} />
			<Text style={tw`text-sm font-medium text-ink`}>{name}</Text>
		</Card>
	);
};

export default FiltersList;
