import { useState } from 'react';

// this hook initializes the explorer state and queries the core
export function useFileExplorer(initialPath = '/') {
  // file explorer hooks maintain their own local state relative to exploration
  const [setPath] = useState(initialPath);
  const [setLocationId] = useState(initialPath);

  //   const { data: volumes } = useQuery(['sys_get_volumes'], () => bridge('sys_get_volumes'));

  return { setPath, setLocationId };
}

// export function useVolumes() {
//   return useQuery(['SysGetVolumes'], () => bridge('SysGetVolumes'));
// }
