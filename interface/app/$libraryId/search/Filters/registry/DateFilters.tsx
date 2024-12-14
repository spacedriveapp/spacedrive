import type {} from '@sd/client'; // required for type inference of createDateRangeFilter

import {
	Calendar,
	CalendarDot,
	CalendarDots,
	CalendarPlus,
	CalendarStar,
	Camera,
	ClockCounterClockwise
} from '@phosphor-icons/react';
import i18n from '~/app/I18n';

import { FilterOption } from '..';
import { SearchOptionSubMenu } from '../../SearchOptions';
import { FilterOptionList } from '../components/FilterOptionList';
import { createDateRangeFilter } from '../factories/createDateRangeFilter';

export const useCommonDateOptions = (): FilterOption[] => {
	return [
		{
			name: i18n.t('Today'),
			value: { from: new Date(new Date().setHours(0, 0, 0, 0)).toISOString() },
			icon: ClockCounterClockwise
		},
		{
			name: i18n.t('Past 7 Days'),
			value: { from: new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString() },
			icon: ClockCounterClockwise
		},
		{
			name: i18n.t('Past 30 Days'),
			value: { from: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString() },
			icon: ClockCounterClockwise
		},
		{
			name: i18n.t('Last Month'),
			value: {
				from: new Date(new Date().getFullYear(), new Date().getMonth() - 1, 1).toISOString()
			},
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
	icon: CalendarStar,
	create: (dateRange) => ({ filePath: { createdAt: dateRange } }),
	extract: (arg) => {
		if ('filePath' in arg && 'createdAt' in arg.filePath) return arg.filePath.createdAt;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: 'custom-date-range',
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
	)
});

export const filePathDateModified = createDateRangeFilter<string>({
	name: i18n.t('Date Modified'),
	translationKey: 'dateModified',
	icon: CalendarDots,
	create: (dateRange) => ({ filePath: { modifiedAt: dateRange } }),
	extract: (arg) => {
		if ('filePath' in arg && 'modifiedAt' in arg.filePath) return arg.filePath.modifiedAt;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: value,
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
	)
});

export const filePathDateIndexed = createDateRangeFilter<string>({
	name: i18n.t('Date Indexed'),
	translationKey: 'dateIndexed',
	icon: CalendarPlus,
	create: (dateRange) => ({ filePath: { indexedAt: dateRange } }),
	extract: (arg) => {
		if ('filePath' in arg && 'indexedAt' in arg.filePath) return arg.filePath.indexedAt;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: value,
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
	)
});

export const objectDateAccessed = createDateRangeFilter<string>({
	name: i18n.t('Date Last Accessed'),
	translationKey: 'dateLastAccessed',
	icon: CalendarDot,
	create: (dateRange) => ({ object: { dateAccessed: dateRange } }),
	extract: (arg) => {
		if ('object' in arg && 'dateAccessed' in arg.object) return arg.object.dateAccessed;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: value,
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
	)
});

export const mediaDateTaken = createDateRangeFilter<string>({
	name: i18n.t('Date Taken'),
	translationKey: 'dateTaken',
	icon: Camera,
	create: (dateRange) => ({ object: { dateAccessed: dateRange } }),
	extract: (arg) => {
		if ('object' in arg && 'dateAccessed' in arg.object) return arg.object.dateAccessed;
	},
	argsToFilterOptions: (dateRange) => {
		return dateRange.map((value) => ({
			name: value,
			value: value
		}));
	},
	useOptions: (): FilterOption[] => useCommonDateOptions(),
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
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
