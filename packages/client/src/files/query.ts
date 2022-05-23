import { useState } from 'react';
import { useQuery } from 'react-query';

import { useBridgeCommand, useBridgeQuery } from '../bridge';
import { useFileExplorerState } from './state';

// this hook initializes the explorer state and queries the core
export function useFileExplorer(initialPath = '/', initialLocation: number | null = null) {
	const fileState = useFileExplorerState();
	// file explorer hooks maintain their own local state relative to exploration
	const [path, setPath] = useState(initialPath);
	const [locationId, setLocationId] = useState(initialPath);

	//   const { data: volumes } = useQuery(['sys_get_volumes'], () => bridge('sys_get_volumes'));

	return { setPath, setLocationId };
}

// export function useVolumes() {
//   return useQuery(['SysGetVolumes'], () => bridge('SysGetVolumes'));
// }
