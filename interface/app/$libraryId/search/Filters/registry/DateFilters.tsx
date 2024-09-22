import type {} from '@sd/client'; // required for type inference of createDateRangeFilter

import { Calendar } from '@phosphor-icons/react';
import i18n from '~/app/I18n';

import { FilterOption } from '..';
import { FilterOptionList } from '../components/FilterOptionList';
import { createDateRangeFilter } from '../factories/createDateRangeFilter';

export const useCommonDateOptions = (): FilterOption[] => {
	return [
		{
			name: i18n.t('Last 7 Days'),
			value: { from: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString() },
			icon: Calendar
		},
		{
			name: i18n.t('Last 30 Days'),
			value: { from: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString() },
			icon: Calendar
		},
		{
			name: i18n.t('This Year'),
			value: { from: new Date(new Date().getFullYear(), 0, 1).toISOString() },
			icon: Calendar
		}
	];
};

export const filePathDateCreated = createDateRangeFilter<string>({
	name: i18n.t('Date Created'),
	translationKey: 'dateCreated',
	icon: Calendar,
	create: (dateRange) => ({ filePath: { createdAt: dateRange } }),
	extract: (arg) => {
		if ('filePath' in arg && 'createdAt' in arg.filePath) return arg.filePath.createdAt;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: value,
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<FilterOptionList filter={filter} options={options} search={search} />
	)
});

// export const filePathDateModified = createDateRangeFilter({});
// export const filePathDateAccessed = createDateRangeFilter({});
// export const objectDateAccessed = createDateRangeFilter({});

// export const dateFilters = [
// 	filePathDateCreated,
// 	filePathDateModified,
// 	filePathDateAccessed
// ] as const;
