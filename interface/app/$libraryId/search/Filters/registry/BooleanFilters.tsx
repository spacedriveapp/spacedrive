import { Heart, SelectionSlash } from '@phosphor-icons/react';
import i18n from '~/app/I18n';

import { FilterOptionBoolean } from '../components/FilterOptionBoolean';
import { createBooleanFilter } from '../factories/createBooleanFilter';

// Hidden Filter
export const hiddenFilter = createBooleanFilter({
	name: i18n.t('hidden'),
	translationKey: 'hidden',
	icon: SelectionSlash,
	extract: (arg) => {
		if ('filePath' in arg && 'hidden' in arg.filePath) return arg.filePath.hidden;
	},
	create: (hidden) => ({ filePath: { hidden } }),
	useOptions: () => [{ name: 'Hidden', value: true, icon: SelectionSlash }],
	Render: ({ filter, options, search }) => <FilterOptionBoolean filter={filter} search={search} />
});

// Favorite Filter
export const favoriteFilter = createBooleanFilter({
	name: i18n.t('favorite'),
	translationKey: 'favorite',
	icon: Heart,
	extract: (arg) => {
		if ('object' in arg && 'favorite' in arg.object) return arg.object.favorite;
	},
	create: (favorite) => ({ object: { favorite } }),
	useOptions: () => [{ name: 'Favorite', value: true, icon: Heart }],
	Render: ({ filter, options, search }) => <FilterOptionBoolean filter={filter} search={search} />
});
