export const SavedSearches = () => {
	// const savedSearches = useSavedSearches();

	return (
		<>
			{/* {savedSearches.searches.length > 0 && (
				<Section
					name="Saved"
					// actionArea={
					// 	<Link to="settings/library/saved-searches">
					// 		<SubtleButton />
					// 	</Link>
					// }
				>
					<SeeMore
						items={savedSearches.searches}
						renderItem={(search) => (
							<SidebarLink
								className="group/button relative w-full"
								to={`search/${search.id}`}
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
									onClick={() => savedSearches.removeSearch(search.id)}
								>
									<X weight="bold" className="text-ink-dull/50" />
								</Button>
							</SidebarLink>
						)}
					/>
				</Section>
			)} */}
		</>
	);
};
