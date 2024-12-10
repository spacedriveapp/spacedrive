import { SearchFilterCRUD } from '..';
import { SearchOptionItem } from '../../SearchOptions';
import { UseSearch } from '../../useSearch';
import { getKey } from '../store';

export const FilterOptionBoolean = ({
	filter,
	search
}: {
	filter: SearchFilterCRUD;
	search: UseSearch<any>;
}) => {
	const { allFiltersKeys } = search;

	const key = getKey({
		type: filter.name,
		name: filter.name,
		value: true
	});

	return (
		<SearchOptionItem
			icon={filter.icon}
			selected={allFiltersKeys?.has(key)}
			setSelected={() => {
				search.setFilters?.((filters = []) => {
					const index = filters.findIndex((f) => filter.extract(f) !== undefined);

					if (index !== -1) {
						filters.splice(index, 1);
					} else {
						const arg = filter.create(true);
						filters.push(arg);
					}

					return filters;
				});
			}}
		>
			{filter.name}
		</SearchOptionItem>
	);
};
