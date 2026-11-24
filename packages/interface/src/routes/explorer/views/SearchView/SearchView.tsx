import { useExplorer } from '../../context';
import { GridView } from '../GridView';
import { ListView } from '../ListView';
import { MediaView } from '../MediaView';
import { ColumnView } from '../ColumnView';
import { SizeView } from '../SizeView';
import { KnowledgeView } from '../KnowledgeView';
import { ContentHits } from './ContentHits';

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
 * file results, since they point at moments rather than whole files.
 */
export function SearchView() {
	const explorer = useExplorer();
	const { viewMode, mode } = explorer;

	// Only render if we're in search mode
	if (mode.type !== 'search') {
		return null;
	}

	return (
		<div className="flex h-full flex-col">
			<ContentHits />
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
