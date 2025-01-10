import { useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { libraryClient } from '@sd/client';
import { getPathIdsPerLocation, useExplorerSearchParams } from '~/app/$libraryId/Explorer/util';
import { isNonEmptyObject } from '~/util';
import { FileDropEvent } from '~/util/events';

import { useQuickRescan } from './useQuickRescan';

export const useFileDropEventHandler = (libraryId?: string) => {
	const navigate = useNavigate();
	const rescan = useQuickRescan();
	const regex = new RegExp(
		'/[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}/location/'
	);
	const id = parseInt(useLocation().pathname.replace(regex, ''));
	const [{ path }] = useExplorerSearchParams();

	useEffect(() => {
		const handler = async (e: FileDropEvent) => {
			e.preventDefault();
			const paths = e.detail.paths;

			if (libraryId && path) {
				libraryClient.mutation([
					'ephemeralFiles.cutFiles',
					{ sources: paths, target_dir: path! }
				]);
			} else if (libraryId) {
				// Get Materialized Path using the location id
				const locationId = id;
				const location = await libraryClient.query(['locations.get', locationId]);
				const locationPath = location!.path;
				libraryClient.mutation([
					'ephemeralFiles.cutFiles',
					{ sources: paths, target_dir: locationPath! }
				]);
			}
		};

		document.addEventListener('filedrop', handler);
		return () => document.removeEventListener('filedrop', handler);
	}, [navigate, libraryId, rescan, id, path]);
};
