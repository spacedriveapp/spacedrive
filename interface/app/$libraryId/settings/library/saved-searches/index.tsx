import { Trash } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useMemo, useState } from 'react';
import {
	SavedSearch,
	SearchFilterArgs,
	useLibraryMutation,
	useLibraryQuery,
	useZodForm
} from '@sd/client';
import { Button, Card, Form, InputField, Label, Tooltip, z } from '@sd/ui';
import { SearchContextProvider, useSearch, useStaticSource } from '~/app/$libraryId/search';
import { AppliedFilters } from '~/app/$libraryId/search/AppliedFilters';
import { Heading } from '~/app/$libraryId/settings/Layout';
import { useDebouncedFormWatch, useLocale } from '~/hooks';

export const Component = () => {
	const savedSearches = useLibraryQuery(['search.saved.list'], { suspense: true });

	const [selectedSearchId, setSelectedSearchId] = useState<number | null>(
		savedSearches.data![0]?.id ?? null
	);

	const selectedSearch = useMemo(() => {
		if (selectedSearchId === null) return null;

		return savedSearches.data!.find((s) => s.id == selectedSearchId) ?? null;
	}, [selectedSearchId, savedSearches.data]);

	const { t } = useLocale();

	return (
		<>
			<Heading title="Saved Searches" description="Manage your saved searches." />
			<div className="flex flex-col gap-4 lg:flex-row">
				<Card className="flex min-w-56 flex-col gap-2 !px-2">
					{savedSearches.data?.map((search) => (
						<button
							onClick={() => setSelectedSearchId(search.id)}
							key={search.id}
							className={clsx(
								'w-full rounded px-1.5 py-0.5 text-left',
								selectedSearch?.id === search.id && 'ring'
							)}
						>
							<span className="text-xs text-white drop-shadow-md">{search.name}</span>
						</button>
					))}
				</Card>
				{selectedSearch ? (
					<EditForm
						key={selectedSearch.id}
						savedSearch={selectedSearch}
						onDelete={() => setSelectedSearchId(null)}
					/>
				) : (
					<div className="text-sm font-medium text-gray-400">
						{t('no_search_selected')}
					</div>
				)}
			</div>
		</>
	);
};

const schema = z.object({
	name: z.string()
});

function EditForm({ savedSearch, onDelete }: { savedSearch: SavedSearch; onDelete: () => void }) {
	const { t } = useLocale();

	const updateSavedSearch = useLibraryMutation('search.saved.update');
	const deleteSavedSearch = useLibraryMutation('search.saved.delete');

	const form = useZodForm({
		schema,
		mode: 'onChange',
		defaultValues: {
			name: savedSearch.name ?? ''
		},
		reValidateMode: 'onChange'
	});

	useDebouncedFormWatch(form, (data) => {
		updateSavedSearch.mutate([savedSearch.id, { name: data.name ?? '' }]);
	});

	const filters = useMemo(() => {
		if (savedSearch.filters === null) return [];

		return JSON.parse(savedSearch.filters) as SearchFilterArgs[];
	}, [savedSearch.filters]);

	const search = useSearch({
		source: useStaticSource({
			search: savedSearch.search ?? '',
			filters,
			target: 'paths'
		})
	});

	return (
		<Form form={form}>
			<div className="flex flex-col gap-4">
				<div className="flex flex-row items-end gap-2">
					<InputField label={t('name')} {...form.register('name')} />
					<Button
						variant="gray"
						className="h-[38px]"
						disabled={deleteSavedSearch.isPending}
						onClick={async () => {
							await deleteSavedSearch.mutateAsync(savedSearch.id);
							onDelete();
						}}
					>
						<Tooltip label={t('delete_tag')}>
							<Trash className="size-4" />
						</Tooltip>
					</Button>
				</div>
				<div className="flex flex-col gap-1">
					<Label className="font-medium">{t('filters')}</Label>
					<div className="flex flex-col items-start gap-2">
						<SearchContextProvider search={search}>
							<AppliedFilters />
						</SearchContextProvider>
					</div>
				</div>
			</div>
		</Form>
	);
}
