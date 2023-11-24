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
import { SearchContext, useSearch } from '~/app/$libraryId/Search';
import { AppliedFilters } from '~/app/$libraryId/Search/AppliedFilters';
import { Heading } from '~/app/$libraryId/settings/Layout';
import { useDebouncedFormWatch } from '~/hooks';

export const Component = () => {
	const savedSearches = useLibraryQuery(['search.saved.list'], { suspense: true });

	const [selectedSearchId, setSelectedSearchId] = useState<number | null>(
		savedSearches.data![0]?.id ?? null
	);

	const selectedSearch = useMemo(() => {
		if (selectedSearchId === null) return null;

		return savedSearches.data!.find((s) => s.id == selectedSearchId) ?? null;
	}, [selectedSearchId, savedSearches.data]);

	return (
		<>
			<Heading title="Saved Searches" description="Manage your saved searches." />
			<div className="flex flex-col gap-4 lg:flex-row">
				<Card className="flex min-w-[14rem] flex-col gap-2 !px-2">
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
					<div className="text-sm font-medium text-gray-400">No Search Selected</div>
				)}
			</div>
		</>
	);
};

const schema = z.object({
	name: z.string()
});

function EditForm({ savedSearch, onDelete }: { savedSearch: SavedSearch; onDelete: () => void }) {
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

	const fixedFilters = useMemo(() => {
		if (savedSearch.filters === null) return [];

		return JSON.parse(savedSearch.filters) as SearchFilterArgs[];
	}, [savedSearch.filters]);

	const search = useSearch({ fixedFilters });

	return (
		<Form form={form}>
			<div className="flex flex-col gap-4">
				<div className="flex flex-row items-end gap-2">
					<InputField label="Name" {...form.register('name')} />
					<Button
						variant="gray"
						className="h-[38px]"
						disabled={deleteSavedSearch.isLoading}
						onClick={async () => {
							await deleteSavedSearch.mutateAsync(savedSearch.id);
							onDelete();
						}}
					>
						<Tooltip label="Delete Tag">
							<Trash className="h-4 w-4" />
						</Tooltip>
					</Button>
				</div>
				<div className="flex flex-col gap-2">
					<Label>Filters</Label>
					<div className="flex flex-col items-start gap-2">
						<SearchContext.Provider value={search}>
							<AppliedFilters />
						</SearchContext.Provider>
					</div>
				</div>
			</div>
		</Form>
	);
}
