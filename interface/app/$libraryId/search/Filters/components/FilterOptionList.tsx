import { SearchFilterCRUD } from '..';
import { SearchOptionItem, SearchOptionSubMenu } from '../../SearchOptions';
import { FilterOption, getKey } from '../../store';
import { UseSearch } from '../../useSearch';
import { useToggleOptionSelected } from '../hooks/useToggleOptionSelected';

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
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
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
		</SearchOptionSubMenu>
	);
};
