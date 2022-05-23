export type ClientQuery =
	| { key: 'ClientGetState' }
	| { key: 'SysGetVolumes' }
	| { key: 'LibGetTags' }
	| { key: 'JobGetRunning' }
	| { key: 'JobGetHistory' }
	| { key: 'SysGetLocations' }
	| { key: 'SysGetLocation'; params: { id: number } }
	| { key: 'LibGetExplorerDir'; params: { location_id: number; path: string; limit: number } }
	| { key: 'GetLibraryStatistics' };
