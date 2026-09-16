import {useLayoutEffect} from 'react';
import {useExplorer} from '../explorer/context';
import {ExplorerView} from '../explorer/ExplorerView';

/**
 * Media sources render through the standard explorer. Source mode projects
 * records through sources.media_listing, with Media as the route default and
 * any later view choice saved only for this source.
 */
export function SourceMediaView({sourceId}: {sourceId: string}) {
	const {enterSourceMode, exitSourceMode} = useExplorer();

	useLayoutEffect(() => {
		enterSourceMode(sourceId);
		return () => {
			exitSourceMode();
		};
	}, [sourceId, enterSourceMode, exitSourceMode]);

	return <ExplorerView defaultViewMode="media" />;
}
