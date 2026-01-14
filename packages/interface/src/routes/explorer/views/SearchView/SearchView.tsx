import {CaretDown} from '@phosphor-icons/react';
import type {
	File,
	FileSearchInput,
	FileSearchOutput,
	SearchFilters,
	SortDirection,
	SortField
} from '@sd/ts-client';
import {flexRender} from '@tanstack/react-table';
import {useVirtualizer} from '@tanstack/react-virtual';
import clsx from 'clsx';
import {
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState
} from 'react';
import {useNormalizedQuery} from '../../../../contexts/SpacedriveContext';
import {useExplorer} from '../../context';
import {useSelection} from '../../SelectionContext';
import {FileCard} from '../GridView/FileCard';
import {TableRow} from '../ListView/TableRow';
import {
	ROW_HEIGHT,
	TABLE_HEADER_HEIGHT,
	TABLE_PADDING_X,
	TABLE_PADDING_Y,
	useTable
} from '../ListView/useTable';

export function SearchView() {
	const explorer = useExplorer();
	const {restoreSelectionFromFiles} = useSelection();

	// Move all hooks before any early returns
	const isSearchMode = explorer.mode.type === 'search';
	const mode = explorer.mode;
	const query = mode.type === 'search' ? mode.query : '';
	const scope = mode.type === 'search' ? mode.scope : ('library' as const);
	const {viewMode, sortBy, currentPath, searchFilters} = explorer;

	// Map explorer sortBy to search SortField
	const searchSortField = useMemo((): SortField => {
		if (!sortBy) return 'Relevance';
		// Map DirectorySortBy/MediaSortBy to SortField
		const sortMap: Record<string, SortField> = {
			name: 'Name',
			size: 'Size',
			modified: 'ModifiedAt',
			type: 'Relevance' // Type doesn't map directly, use Relevance
		};
		return sortMap[sortBy] || 'Relevance';
	}, [sortBy]);

	// Default to Desc for search (most relevant first)
	const searchSortDirection: SortDirection = 'Desc';

	const defaultFilters: SearchFilters = {
		file_types: null,
		tags: null,
		date_range: null,
		size_range: null,
		locations: null,
		content_types: null,
		include_hidden: null,
		include_archived: null
	};

	// Convert explorer SearchFilters to ts-client SearchFilters
	const convertedFilters = useMemo<SearchFilters>(() => {
		if (!searchFilters) return defaultFilters;
		// For now, return default filters as explorer filters have different structure
		// TODO: Implement proper conversion when search filters are fully implemented
		return defaultFilters;
	}, [searchFilters]);

	const searchQueryInput = useMemo<FileSearchInput>(
		() => ({
			query,
			scope:
				scope === 'folder' && currentPath
					? {Path: {path: currentPath}}
					: 'Library',
			filters: convertedFilters,
			mode: 'Normal',
			sort: {
				field: searchSortField,
				direction: searchSortDirection
			},
			pagination: {
				limit: 1000,
				offset: 0
			}
		}),
		[query, scope, currentPath, convertedFilters, searchSortField]
	);

	const searchQuery = useNormalizedQuery<FileSearchInput, FileSearchOutput>({
		wireMethod: 'query:search.files',
		input: searchQueryInput,
		resourceType: 'file',
		// Note: pathScope may have type mismatch (device_slug vs device_id)
		// This is handled by the backend which accepts both formats
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		pathScope:
			scope === 'folder' && currentPath
				? (currentPath as any)
				: undefined,
		enabled: isSearchMode && query.length >= 2,
		debug: false
	});

	// Properly typed files extraction with memoization
	const files = useMemo(() => {
		return (searchQuery.data as FileSearchOutput | undefined)?.files || [];
	}, [searchQuery.data]);

	useEffect(() => {
		explorer.setCurrentFiles(files);
	}, [files, explorer.setCurrentFiles]);

	// Restore selection when files load (for tab switching)
	useEffect(() => {
		restoreSelectionFromFiles(files);
	}, [files, restoreSelectionFromFiles]);

	// Early returns after all hooks
	if (!isSearchMode) {
		return null;
	}

	if (query.length < 2) {
		return (
			<div className="flex h-full flex-col items-center justify-center p-8 text-center">
				<p className="text-ink-dull text-sm">
					Type at least 2 characters to search
				</p>
			</div>
		);
	}

	if (searchQuery.isLoading) {
		return (
			<div className="flex h-full flex-col items-center justify-center p-8 text-center">
				<p className="text-ink-dull text-sm">Searching...</p>
			</div>
		);
	}

	if (searchQuery.isError) {
		return (
			<div className="flex h-full flex-col items-center justify-center p-8 text-center">
				<p className="text-ink-dull mb-2">Search failed</p>
				<p className="text-ink-faint text-sm">
					{searchQuery.error?.message ||
						'An error occurred while searching'}
				</p>
			</div>
		);
	}

	if (files.length === 0) {
		return (
			<div className="flex h-full flex-col items-center justify-center p-8 text-center">
				<p className="text-ink-dull mb-2">No results found</p>
				<p className="text-ink-faint text-sm">
					Try a different search term or adjust your filters
				</p>
			</div>
		);
	}

	if (viewMode === 'grid') {
		return <SearchGridView files={files} />;
	}

	if (viewMode === 'list') {
		return <SearchListView files={files} />;
	}

	return (
		<div className="flex h-full flex-col items-center justify-center p-8 text-center">
			<p className="text-ink-dull text-sm">
				Search results in {viewMode} view coming soon
			</p>
		</div>
	);
}

function SearchGridView({files}: {files: File[]}) {
	const {isSelected, focusedIndex, selectFile, selectedFiles} =
		useSelection();
	const explorer = useExplorer();
	const {gridSize, gapSize} = explorer.viewSettings;

	const containerRef = useRef<HTMLDivElement>(null);
	const [containerWidth, setContainerWidth] = useState(0);

	useLayoutEffect(() => {
		const updateWidth = () => {
			if (containerRef.current) {
				setContainerWidth(containerRef.current.offsetWidth);
			}
		};
		updateWidth();
		window.addEventListener('resize', updateWidth);
		return () => window.removeEventListener('resize', updateWidth);
	}, []);

	const padding = 24;
	const itemWidth = gridSize;
	const itemHeight = gridSize + 40;
	const columnsCount = Math.max(
		1,
		Math.floor(
			(containerWidth - padding * 2 + gapSize) / (itemWidth + gapSize)
		)
	);

	const virtualizer = useVirtualizer({
		count: Math.ceil(files.length / columnsCount),
		getScrollElement: () => containerRef.current,
		estimateSize: () => itemHeight + gapSize,
		overscan: 3
	});

	return (
		<div ref={containerRef} className="h-full overflow-auto px-6 py-4">
			<div
				style={{
					height: `${virtualizer.getTotalSize()}px`,
					position: 'relative'
				}}
			>
				{virtualizer.getVirtualItems().map((virtualRow) => {
					const startIdx = virtualRow.index * columnsCount;
					const rowFiles = files.slice(
						startIdx,
						startIdx + columnsCount
					);

					return (
						<div
							key={virtualRow.key}
							style={{
								position: 'absolute',
								top: 0,
								left: 0,
								width: '100%',
								transform: `translateY(${virtualRow.start}px)`
							}}
						>
							<div
								style={{
									display: 'grid',
									gridTemplateColumns: `repeat(${columnsCount}, ${itemWidth}px)`,
									gap: `${gapSize}px`
								}}
							>
								{rowFiles.map((file, colIndex) => {
									const fileIndex = startIdx + colIndex;
									return (
										<FileCard
											key={file.id}
											file={file}
											fileIndex={fileIndex}
											allFiles={files}
											selected={isSelected(file.id)}
											focused={focusedIndex === fileIndex}
											selectedFiles={selectedFiles}
											selectFile={selectFile}
										/>
									);
								})}
							</div>
						</div>
					);
				})}
			</div>
		</div>
	);
}

function SearchListView({files}: {files: File[]}) {
	const {focusedIndex, isSelected, selectFile, selectedFileIds} =
		useSelection();

	const containerRef = useRef<HTMLDivElement>(null);
	const headerScrollRef = useRef<HTMLDivElement>(null);
	const bodyScrollRef = useRef<HTMLDivElement>(null);

	// Fix: useTable takes files directly, returns { table, columns }
	const {table} = useTable(files);
	const {rows} = table.getRowModel();
	const headerGroups = table.getHeaderGroups();
	const totalWidth = table.getTotalSize();

	const virtualizer = useVirtualizer({
		count: files.length,
		getScrollElement: () => bodyScrollRef.current,
		estimateSize: () => ROW_HEIGHT,
		overscan: 10
	});

	const handleBodyScroll = useCallback(() => {
		if (bodyScrollRef.current && headerScrollRef.current) {
			headerScrollRef.current.scrollLeft =
				bodyScrollRef.current.scrollLeft;
		}
	}, []);

	// Header click handler for sorting (search doesn't support client-side sorting,
	// but we can show the UI for future server-side sorting)
	const handleHeaderClick = useCallback(() => {
		// For now, search sorting is handled server-side via the query
		// This could be extended to update the search query with new sort options
	}, []);

	const handleHeaderKeyDown = useCallback(
		(e: React.KeyboardEvent) => {
			if (e.key === 'Enter' || e.key === ' ') {
				e.preventDefault();
				handleHeaderClick();
			}
		},
		[handleHeaderClick]
	);

	return (
		<div ref={containerRef} className="flex h-full flex-col">
			<div
				ref={headerScrollRef}
				className="overflow-hidden"
				style={{
					paddingLeft: TABLE_PADDING_X,
					paddingRight: TABLE_PADDING_X
				}}
			>
				<div
					style={{
						width: totalWidth,
						height: TABLE_HEADER_HEIGHT
					}}
					className="border-sidebar-line/30 flex items-center border-b"
				>
					{headerGroups.map((headerGroup) =>
						headerGroup.headers.map((header) => {
							const canSort = header.column.getCanSort();
							return canSort ? (
								<button
									key={header.id}
									type="button"
									style={{width: header.getSize()}}
									className={clsx(
										'text-sidebar-inkDull flex select-none items-center gap-1 px-3 text-xs font-medium',
										'hover:text-sidebar-ink cursor-pointer'
									)}
									onClick={handleHeaderClick}
									onKeyDown={handleHeaderKeyDown}
								>
									{flexRender(
										header.column.columnDef.header,
										header.getContext()
									)}
									{header.column.getIsSorted() && (
										<div
											className={clsx(
												'size-3 transition-transform',
												header.column.getIsSorted() ===
													'asc' && 'rotate-180'
											)}
										>
											<CaretDown weight="bold" />
										</div>
									)}
								</button>
							) : (
								<div
									key={header.id}
									style={{width: header.getSize()}}
									className="text-sidebar-inkDull flex select-none items-center gap-1 px-3 text-xs font-medium"
								>
									{flexRender(
										header.column.columnDef.header,
										header.getContext()
									)}
								</div>
							);
						})
					)}
				</div>
			</div>

			<div
				ref={bodyScrollRef}
				onScroll={handleBodyScroll}
				className="flex-1 overflow-auto"
				style={{
					paddingLeft: TABLE_PADDING_X,
					paddingRight: TABLE_PADDING_X,
					paddingTop: TABLE_PADDING_Y,
					paddingBottom: TABLE_PADDING_Y
				}}
			>
				<div
					style={{
						height: `${virtualizer.getTotalSize()}px`,
						position: 'relative'
					}}
				>
					{virtualizer.getVirtualItems().map((virtualRow) => {
						const file = files[virtualRow.index];
						const row = rows[virtualRow.index];
						if (!row || !file) return null;

						const fileIsSelected = isSelected(file.id);
						const isFocused = focusedIndex === virtualRow.index;
						const previousRow = rows[virtualRow.index - 1];
						const nextRow = rows[virtualRow.index + 1];
						const isPreviousSelected = previousRow
							? selectedFileIds.has(previousRow.original.id)
							: false;
						const isNextSelected = nextRow
							? selectedFileIds.has(nextRow.original.id)
							: false;

						return (
							<div
								key={virtualRow.key}
								style={{
									position: 'absolute',
									top: 0,
									left: 0,
									width: '100%',
									height: `${virtualRow.size}px`,
									transform: `translateY(${virtualRow.start}px)`
								}}
							>
								<TableRow
									row={row}
									file={file}
									files={files}
									index={virtualRow.index}
									isSelected={fileIsSelected}
									isFocused={isFocused}
									isPreviousSelected={isPreviousSelected}
									isNextSelected={isNextSelected}
									measureRef={virtualizer.measureElement}
									selectFile={selectFile}
								/>
							</div>
						);
					})}
				</div>
			</div>
		</div>
	);
}
