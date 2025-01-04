// Import icons
import { Folder } from '@phosphor-icons/react';
import { useLibraryQuery } from '@sd/client';
import i18n from '~/app/I18n';

import { SearchOptionSubMenu } from '../../SearchOptions';
import { FilterOptionList } from '../components/FilterOptionList';
import { createInOrNotInFilter } from '../factories/createInOrNotInFilter';

export const locationFilter = createInOrNotInFilter<number>({
	name: i18n.t('location'),
	translationKey: 'location',
	icon: Folder,
	create: (locations) => ({ filePath: { locations } }),
	extract: (arg) => {
		if ('filePath' in arg && 'locations' in arg.filePath) return arg.filePath.locations;
	},
	argsToFilterOptions(values, options) {
		return values
			.map((value) => {
				const option = options.get(this.name)?.find((o) => o.value === value);
				if (!option) return;
				return {
					...option,
					type: this.name
				};
			})
			.filter(Boolean) as any;
	},
	useOptions: () => {
		const query = useLibraryQuery(['locations.list'], { keepPreviousData: true });
		const locations = query.data;

		return (locations ?? []).map((location) => ({
			name: location.name!,
			value: location.id,
			icon: 'Folder'
		}));
	},
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList filter={filter} options={options} search={search} />
		</SearchOptionSubMenu>
	)
});
