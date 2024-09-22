import { SearchFilterCRUD } from '..';
import { FilterOption } from '../';
import { UseSearch } from '../../useSearch';

export function useToggleOptionSelected({ search }: { search: UseSearch<any> }) {
	return ({
		filter,
		option,
		select
	}: {
		filter: SearchFilterCRUD;
		option: FilterOption;
		select: boolean;
	}) => {
		search.setFilters?.((filters = []) => {
			const rawArg = filters.find((arg) => filter.extract(arg));

			if (!rawArg) {
				const arg = filter.create(option.value);
				filters.push(arg);
			} else {
				const rawArgIndex = filters.findIndex((arg) => filter.extract(arg))!;

				const arg = filter.extract(rawArg)!;

				if (select) {
					if (rawArg) filter.applyAdd(arg, option);
				} else {
					if (!filter.applyRemove(arg, option)) filters.splice(rawArgIndex, 1);
				}
			}

			return filters;
		});
	};
}
