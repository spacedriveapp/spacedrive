import type {} from '@sd/client'; // required for type inference of createDateRangeFilter

import { Textbox } from '@phosphor-icons/react';
import i18n from '~/app/I18n';

import { createInOrNotInFilter } from '../factories/createInOrNotInFilter';
import { createTextMatchFilter } from '../factories/createTextMatchFilter';

// Name Filter
export const nameFilter = createTextMatchFilter({
	name: i18n.t('name'),
	translationKey: 'name',
	icon: Textbox,
	extract: (arg) => {
		if ('filePath' in arg && 'name' in arg.filePath) return arg.filePath.name;
	},
	create: (name) => ({ filePath: { name } }),
	useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
	Render: ({ filter, search }) => <></>
});

// Extension Filter
export const extensionFilter = createInOrNotInFilter({
	name: i18n.t('extension'),
	translationKey: 'extension',
	icon: Textbox,
	extract: (arg) => {
		if ('filePath' in arg && 'extension' in arg.filePath) return arg.filePath.extension;
	},
	create: (extension) => ({ filePath: { extension } }),
	useOptions: ({ search }) => [{ name: search, value: search, icon: Textbox }],
	Render: ({ filter, search }) => <></>
});
