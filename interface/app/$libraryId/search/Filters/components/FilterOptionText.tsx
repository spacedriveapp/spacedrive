import { useState } from 'react';
import { Button, Input, Select } from '@sd/ui';
import { useLocale } from '~/hooks';

import { FilterTypeCondition, SearchFilterCRUD } from '..';
import { SearchOptionSubMenu } from '../../SearchOptions';
import { UseSearch } from '../../useSearch';
import { getKey } from '../store';

export const FilterOptionText = ({
	filter,
	search
}: {
	filter: SearchFilterCRUD;
	search: UseSearch<any>;
}) => {
	const [value, setValue] = useState('');
	const [matchType, setMatchType] = useState<'contains' | 'startsWith' | 'endsWith' | 'equals'>(
		'contains'
	);

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
				<Select
					value={matchType}
					onChange={setMatchType}
					containerClassName="h-[30px] whitespace-nowrap"
				>
					<option value="contains">{t('contains')}</option>
					<option value="startsWith">{t('starts with')}</option>
					<option value="endsWith">{t('ends with')}</option>
					<option value="equals">{t('equals')}</option>
				</Select>
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
