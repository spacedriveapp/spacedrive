import { SearchFilterCRUD } from '..';
import { SearchOptionItem, SearchOptionSubMenu } from '../../SearchOptions';
import { UseSearch } from '../../useSearch';
import { useToggleOptionSelected } from '../hooks/useToggleOptionSelected';
import { FilterOption, getKey } from '../store';

export const FilterOptionList = ({
	filter,
	options,
	search,
	empty
}: {
	filter: SearchFilterCRUD;
	options: FilterOption[];
	search: UseSearch<any>;
	empty?: () => JSX.Element;
}) => {
	const { allFiltersKeys } = search;

	const toggleOptionSelected = useToggleOptionSelected({ search });

	return (
		<>
			{empty?.() && options.length === 0
				? empty()
				: options?.map((option) => {
						const optionKey = getKey({
							...option,
							type: filter.name
						});

						return (
							<SearchOptionItem
								selected={allFiltersKeys.has(optionKey)}
								setSelected={(value) => {
									toggleOptionSelected({
										filter,
										option,
										select: value
									});
								}}
								key={option.value}
								icon={option.icon}
							>
								{option.name}
							</SearchOptionItem>
						);
					})}
		</>
	);
};
