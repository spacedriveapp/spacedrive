import { useEffect } from 'react';
import { useExplorer } from '../../context';
import { GridView } from '../GridView';
import { ListView } from '../ListView';
import { MediaView } from '../MediaView';
import { ColumnView } from '../ColumnView';
import { SizeView } from '../SizeView';
import { KnowledgeView } from '../KnowledgeView';

/**
 * RecentsView displays recently indexed files sorted by indexed_at timestamp.
 *
 * Similar to SearchView, it delegates to existing view components which automatically
 * read from useExplorerFiles. This ensures recents has the same interactions as normal
 * browsing: keyboard navigation, drag-to-select, context menus, etc.
 */
export function RecentsView() {
	const explorer = useExplorer();
	const { viewMode, enterRecentsMode, exitRecentsMode } = explorer;

	// Enter recents mode on mount, exit on unmount
	useEffect(() => {
		enterRecentsMode();
		return () => exitRecentsMode();
	}, [enterRecentsMode, exitRecentsMode]);

	// Route to the appropriate view based on viewMode
	// The views will automatically use recents results via useExplorerFiles
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
			return <GridView />;
	}
}
