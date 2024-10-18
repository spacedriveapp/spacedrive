import { X } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useMatch, useNavigate, useResolvedPath } from 'react-router';
import { useLibraryMutation, useLibraryQuery, type SavedSearch } from '@sd/client';
import { Button } from '@sd/ui';
import { useExplorerDroppable } from '~/app/$libraryId/Explorer/useExplorerDroppable';
import { Folder } from '~/components';
import { useLocale } from '~/hooks';

import SidebarLink from '../../SidebarLayout/Link';
import Section from '../../SidebarLayout/Section';
import { SeeMore } from '../../SidebarLayout/SeeMore';

export default function SavedSearches() {
	const savedSearches = useLibraryQuery(['search.saved.list']);

	const path = useResolvedPath('saved-search/:id');
	const match = useMatch(path.pathname);
	const currentSearchId = match?.params?.id;

	const currentIndex = currentSearchId
		? savedSearches.data?.findIndex((s) => s.id === Number(currentSearchId))
		: undefined;

	const navigate = useNavigate();

	const { t } = useLocale();

	const deleteSavedSearch = useLibraryMutation(['search.saved.delete'], {
		onSuccess() {
			if (currentIndex !== undefined && savedSearches.data) {
				const nextIndex = Math.min(currentIndex + 1, savedSearches.data.length - 2);

				const search = savedSearches.data[nextIndex];

				if (search) navigate(`saved-search/${search.id}`);
				else navigate(`./`);
			}
		}
	});

	if (!savedSearches.data || savedSearches.data.length < 1) return null;

	return (
		<Section
			name={t('saved_searches')}
			// actionArea={
			// 	<Link to="settings/library/saved-searches">
			// 		<SubtleButton />
			// 	</Link>
			// }
		>
			<SeeMore>
				{savedSearches.data.map((search, i) => (
					<SavedSearch
						key={search.id}
						search={search}
						onDelete={() => deleteSavedSearch.mutate(search.id)}
					/>
				))}
			</SeeMore>
		</Section>
	);
}

const SavedSearch = ({ search, onDelete }: { search: SavedSearch; onDelete(): void }) => {
	const searchId = useMatch('/:libraryId/saved-search/:searchId')?.params.searchId;

	const { isDroppable, className, setDroppableRef } = useExplorerDroppable({
		id: `sidebar-saved-search-${search.id}`,
		allow: ['Path', 'NonIndexedPath', 'Object'],
		disabled: Number(searchId) === search.id,
		navigateTo: `saved-search/${search.id}`
	});

	return (
		<SidebarLink
			ref={setDroppableRef}
			to={`saved-search/${search.id}`}
			className={clsx(
				'group/button relative border border-transparent',
				isDroppable && '!cursor-no-drop',
				className
			)}
		>
			<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
				<Folder size={18} />
			</div>

			<span className="truncate">{search.name}</span>

			<Button
				className="absolute right-1 top-1/2 hidden -translate-y-1/2 rounded-full shadow group-hover/button:block"
				size="icon"
				variant="subtle"
				onClick={(e: React.MouseEvent) => {
					e.preventDefault();
					e.stopPropagation();
					onDelete();
				}}
			>
				<X size={10} weight="bold" className="text-ink-dull/50" />
			</Button>
		</SidebarLink>
	);
};
