import { useExplorer } from '../../context';
import { GridView } from '../GridView';
import { ListView } from '../ListView';
import { MediaView } from '../MediaView';
import { ColumnView } from '../ColumnView';
import { SizeView } from '../SizeView';
import { KnowledgeView } from '../KnowledgeView';
import { Sparkle } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { useSemanticAvailability } from '../../hooks/useSemanticSearch';
import { ContentHits } from './ContentHits';
import { SemanticHits } from './SemanticHits';

const MEANING_KEY = 'sd.explorer.search.meaning';

/**
 * SearchView is a router that delegates to the appropriate view component.
 *
 * Instead of reimplementing rendering logic, it reuses existing view components
 * (GridView, ListView, etc.) which now automatically handle search results via
 * the useExplorerFiles hook.
 *
 * This ensures search has the same interactions as normal browsing:
 * - Keyboard navigation (arrow keys, shift-select)
 * - Drag-to-select
 * - Context menus
 * - Quick preview
 * - All other explorer features
 *
 * Matches inside transcripts, scene descriptions and captions show above the
 * file results, since they point at moments rather than whole files. With the
 * Meaning toggle on, files ranked by what they are about take that place.
 */
export function SearchView() {
	const explorer = useExplorer();
	const { viewMode, mode } = explorer;
	const semantic = useSemanticAvailability();
	const [meaning, setMeaning] = useState(
		() => localStorage.getItem(MEANING_KEY) === '1'
	);
	useEffect(() => {
		localStorage.setItem(MEANING_KEY, meaning ? '1' : '0');
	}, [meaning]);
	const byMeaning = meaning && semantic.available;

	// Only render if we're in search mode
	if (mode.type !== 'search') {
		return null;
	}

	return (
		<div className="flex h-full flex-col">
			<div className="border-app-line flex shrink-0 items-center gap-2 border-b px-4 py-1.5">
				<button
					onClick={() => setMeaning((value) => !value)}
					disabled={!semantic.available}
					aria-pressed={byMeaning}
					title={
						semantic.available
							? 'Find files by what they are about, not only the words typed'
							: (semantic.reason ?? 'Semantic search is unavailable')
					}
					className={clsx(
						'flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium',
						byMeaning
							? 'border-accent bg-accent/15 text-accent'
							: 'border-app-line text-ink-dull hover:text-ink',
						!semantic.available && 'cursor-not-allowed opacity-50 hover:text-ink-dull'
					)}
				>
					<Sparkle size={12} weight={byMeaning ? 'fill' : 'regular'} />
					Meaning
				</button>
				{!semantic.available && !semantic.isLoading && semantic.reason && (
					<span className="text-ink-faint truncate text-xs">{semantic.reason}</span>
				)}
			</div>
			{byMeaning ? <SemanticHits /> : <ContentHits />}
			<div className="relative min-h-0 flex-1">
				<FileResults viewMode={viewMode} />
			</div>
		</div>
	);
}

function FileResults({ viewMode }: { viewMode: string }) {
	// The views will automatically use search results via useExplorerFiles
	switch (viewMode) {
		case 'grid':
			return <GridView />;
		case 'list':
			return <ListView />;
		case 'media':
			return <MediaView />;
		case 'column':
			return <ColumnView />;
		case 'size':
			return <SizeView />;
		case 'knowledge':
			return <KnowledgeView />;
		default:
			return (
				<div className="flex h-full flex-col items-center justify-center p-8 text-center">
					<p className="text-ink-dull text-sm">
						Search in {viewMode} view coming soon
					</p>
				</div>
			);
	}
}
