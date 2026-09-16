import {useEffect, useLayoutEffect} from 'react';
import {useExplorer} from '../../context';
import {ColumnView} from '../ColumnView';
import {GridView} from '../GridView';
import {KnowledgeView} from '../KnowledgeView';
import {ListView} from '../ListView';
import {MediaView} from '../MediaView';
import {SizeView} from '../SizeView';

/**
 * RecentsView displays recently indexed files sorted by indexed_at timestamp.
 *
 * Similar to SearchView, it delegates to existing view components which automatically
 * read from useExplorerFiles. This ensures recents has the same interactions as normal
 * browsing: keyboard navigation, drag-to-select, context menus, etc.
 */
export function RecentsView() {
	const explorer = useExplorer();
	const {
		viewMode,
		enterRecentsMode,
		exitRecentsMode,
		loadPreferencesForSpaceItem
	} = explorer;

	useLayoutEffect(() => {
		loadPreferencesForSpaceItem('recents');
	}, [loadPreferencesForSpaceItem]);

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
