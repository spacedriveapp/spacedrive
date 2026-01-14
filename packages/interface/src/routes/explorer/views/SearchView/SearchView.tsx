import { useExplorer } from '../../context';
import { GridView } from '../GridView';
import { ListView } from '../ListView';
import { MediaView } from '../MediaView';
import { ColumnView } from '../ColumnView';
import { SizeView } from '../SizeView';
import { KnowledgeView } from '../KnowledgeView';

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
 */
export function SearchView() {
	const explorer = useExplorer();
	const { viewMode, mode } = explorer;

	// Only render if we're in search mode
	if (mode.type !== 'search') {
		return null;
	}

	// Show minimum character hint
	if (mode.query.length < 2) {
		return (
			<div className="flex h-full flex-col items-center justify-center p-8 text-center">
				<p className="text-ink-dull text-sm">
					Type at least 2 characters to search
				</p>
			</div>
		);
	}

	// Route to the appropriate view based on viewMode
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
