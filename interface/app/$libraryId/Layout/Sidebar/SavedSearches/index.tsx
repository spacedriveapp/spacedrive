import { Folder, X } from '@phosphor-icons/react';
import { useMatch, useNavigate, useResolvedPath } from 'react-router';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button } from '@sd/ui';

import SidebarLink from '../Link';
import Section from '../Section';
import { SeeMore } from '../SeeMore';

export const SavedSearches = () => {
	const savedSearches = useLibraryQuery(['search.saved.list']);

	const path = useResolvedPath('saved-search/:id');
	const match = useMatch(path.pathname);
	const currentSearchId = match?.params?.id;

	const currentIndex = currentSearchId
		? savedSearches.data?.findIndex((s) => s.id === Number(currentSearchId))
		: undefined;

	const navigate = useNavigate();

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
			name="Saved Searches"
			// actionArea={
			// 	<Link to="settings/library/saved-searches">
			// 		<SubtleButton />
			// 	</Link>
			// }
		>
			<SeeMore>
				{savedSearches.data.map((search, i) => (
					<SidebarLink
						className="group/button relative w-full"
						to={`saved-search/${search.id}`}
						key={search.id}
					>
						<div className="relative -mt-0.5 mr-1 shrink-0 grow-0">
							<Folder size={18} />
						</div>

						<span className="truncate">{search.name}</span>

						<Button
							className="absolute right-[2px] top-[2px] hidden rounded-full shadow group-hover/button:block"
							size="icon"
							variant="subtle"
							onClick={(e: React.MouseEvent) => {
								e.preventDefault();
								e.stopPropagation();

								deleteSavedSearch.mutate(search.id);
							}}
						>
							<X size={10} weight="bold" className="text-ink-dull/50" />
						</Button>
					</SidebarLink>
				))}
			</SeeMore>
		</Section>
	);
};
