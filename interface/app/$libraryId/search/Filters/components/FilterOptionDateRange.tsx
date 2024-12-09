// import { Range } from '@sd/client';

// import { SearchFilterCRUD } from '..';
// import { SearchOptionItem } from '../../SearchOptions';
// import { UseSearch } from '../../useSearch';
// import { getKey } from '../store';

// export const FilterOptionDateRange = ({
// 	filter,
// 	search
// }: {
// 	filter: SearchFilterCRUD;
// 	search: UseSearch<any>;
// }) => {
// 	const { allFiltersKeys } = search;

// 	const key = getKey({
// 		type: filter.name,
// 		name: filter.name,
// 		value: { start: new Date(), end: new Date() } // Example default range
// 	});

// 	return (
// 		<SearchOptionItem
// 			icon={filter.icon}
// 			selected={allFiltersKeys?.has(key)}
// 			setSelected={() => {
// 				search.setFilters?.((filters = []) => {
// 					const index = filters.findIndex((f) => filter.extract(f) !== undefined);

// 					if (index !== -1) {
// 						filters.splice(index, 1);
// 					} else {
// 						const arg = filter.create({ start: new Date(), end: new Date() }); // Example default range
// 						filters.push(arg);
// 					}

// 					return filters;
// 				});
// 			}}
// 		>
// 			{filter.name}
// 		</SearchOptionItem>
// 	);
// };
