import { CircleDashed } from '@phosphor-icons/react';
import { useLibraryQuery } from '@sd/client';
import i18n from '~/app/I18n';

import { SearchOptionSubMenu } from '../../SearchOptions';
import { FilterOptionList } from '../components/FilterOptionList';
import { createInOrNotInFilter } from '../factories/createInOrNotInFilter';

export const tagsFilter = createInOrNotInFilter<number>({
	name: i18n.t('tags'),
	translationKey: 'tag',
	icon: CircleDashed,
	extract: (arg) => {
		if ('object' in arg && 'tags' in arg.object) return arg.object.tags;
	},
	create: (tags) => ({ object: { tags } }),
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
		const query = useLibraryQuery(['tags.list'], { keepPreviousData: true });
		const tags = query.data;

		return (tags ?? []).map((tag) => ({
			name: tag.name!,
			value: tag.id,
			icon: tag.color || 'CircleDashed'
		}));
	},
	Render: ({ filter, options, search }) => (
		<SearchOptionSubMenu name={filter.name} icon={filter.icon}>
			<FilterOptionList
				empty={() => (
					<div className="flex flex-col items-center justify-center gap-2 p-2">
						<span className="icon-tag size-4" />
						<p className="w-4/5 text-center text-xs text-ink-dull">
							{i18n.t('no_tags')}
						</p>
					</div>
				)}
				filter={filter}
				options={options}
				search={search}
			/>
		</SearchOptionSubMenu>
	)
});
