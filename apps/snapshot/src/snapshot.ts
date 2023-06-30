interface SnapshotParams {
	background: string;
	os: 'windows' | 'mac' | 'linux';
	viewport: {
		width: number;
		height: number;
		X: number;
		Y: number;
	};
	appRoute: string;
	dataMethod: 'server' | 'json';
}
