import { useEffect } from 'react';
import { useParams } from 'react-router-dom';
import { useExplorer } from '../explorer/context';
import { ExplorerView } from '../explorer/ExplorerView';

/**
 * Tag view — renders files tagged with a specific tag using the standard explorer view.
 * Activates tag mode on the explorer context so useExplorerFiles fetches from files.by_tag.
 */
export function TagView() {
	const { tagId } = useParams<{ tagId: string }>();
	const { enterTagMode, exitTagMode } = useExplorer();

	useEffect(() => {
		if (tagId) {
			enterTagMode(tagId);
		}
		return () => {
			exitTagMode();
		};
	}, [tagId, enterTagMode, exitTagMode]);

	return <ExplorerView />;
}
