import { useState } from 'react';
import { Button, Input } from '@sd/ui';
import { useLocale } from '~/hooks';

import { SearchFilterCRUD } from '..';
import { SearchOptionSubMenu } from '../../SearchOptions';
import { getKey } from '../../store';
import { UseSearch } from '../../useSearch';

export const FilterOptionText = ({
	filter,
	search
}: {
	filter: SearchFilterCRUD;
	search: UseSearch<any>;
}) => {
	const [value, setValue] = useState('');

	const { allFiltersKeys } = search;
	const key = getKey({
		type: filter.name,
		name: value,
		value
	});

	const { t } = useLocale();

	return (
		<SearchOptionSubMenu className="!p-1.5" name={filter.name} icon={filter.icon}>
			<form
				className="flex gap-1.5"
				onSubmit={(e) => {
					e.preventDefault();
					search.setFilters?.((filters) => {
						if (allFiltersKeys.has(key)) return filters;

						const arg = filter.create(value);
						filters?.push(arg);
						setValue('');

						return filters;
					});
				}}
			>
				<Input className="w-3/4" value={value} onChange={(e) => setValue(e.target.value)} />
				<Button
					disabled={value.length === 0 || allFiltersKeys.has(key)}
					variant="accent"
					className="w-full"
					type="submit"
				>
					{t('apply')}
				</Button>
			</form>
		</SearchOptionSubMenu>
	);
};
