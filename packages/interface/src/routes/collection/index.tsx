import {useLayoutEffect} from 'react';
import {useParams} from 'react-router-dom';
import {useExplorer} from '../explorer/context';
import {ExplorerView} from '../explorer/ExplorerView';

/**
 * Collection view — files identified at index time (screenshots, screen
 * recordings) rendered through the standard explorer. Activates collection
 * mode so useExplorerFiles fetches from files.collection_listing, and
 * defaults to the media grid since collections are visual by nature.
 */
// Collections that are visual by nature open in the media grid; mixed-kind
// collections (recents) keep the user's current view.
const MEDIA_SLUGS = new Set(['screenshots', 'screen-recordings']);

export function CollectionView() {
	const {slug} = useParams<{slug: string}>();
	const {enterCollectionMode, exitCollectionMode} = useExplorer();

	useLayoutEffect(() => {
		if (slug) {
			enterCollectionMode(slug);
		}
		return () => {
			exitCollectionMode();
		};
	}, [slug, enterCollectionMode, exitCollectionMode]);

	return (
		<ExplorerView
			defaultViewMode={slug && MEDIA_SLUGS.has(slug) ? 'media' : 'grid'}
		/>
	);
}
