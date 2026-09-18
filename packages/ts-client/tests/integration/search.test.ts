import { describe, test, expect, beforeAll } from 'bun:test';
import { readFile } from 'fs/promises';
import { SpacedriveClient } from '../../src/client';

interface SearchBridgeConfig {
	socket_addr: string;
	library_id: string;
	device_slug: string;
	source_path: string;
	browsed_dir_path: string;
	test_data_path: string;
}

let bridgeConfig: SearchBridgeConfig;
let client: SpacedriveClient;

beforeAll(async () => {
	// Read bridge config from Rust test
	const configPath = process.env.BRIDGE_CONFIG_PATH;
	if (!configPath) {
		throw new Error('BRIDGE_CONFIG_PATH environment variable not set');
	}

	const configJson = await readFile(configPath, 'utf-8');
	bridgeConfig = JSON.parse(configJson);

	console.log('[TS] Bridge config loaded:', {
		socket: bridgeConfig.socket_addr,
		library: bridgeConfig.library_id,
		source_path: bridgeConfig.source_path,
		browsed_path: bridgeConfig.browsed_dir_path,
	});

	// Connect to daemon via TCP socket
	client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);
	client.setCurrentLibrary(bridgeConfig.library_id);

	console.log('[TS] Connected to daemon');
});

describe('Search - Tracked Source', () => {
	test('should find a file by name across the library', async () => {
		console.log('[TS] Testing library-wide search for "report"...');

		const searchInput = {
			query: 'report',
			scope: 'Library',
			mode: 'Normal',
			filters: {},
			sort: {
				field: 'Relevance',
				direction: 'Desc',
			},
			pagination: {
				limit: 50,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
			execution_time_ms: result.execution_time_ms,
		});

		// Debug: print all results
		if (result.results.length > 0) {
			console.log('[TS] Found files:');
			result.results.forEach((r: any, i: number) => {
				console.log(`  ${i + 1}. ${r.file.name} (score: ${r.score})`);
			});
		}

		// Assertions
		expect(result.total_found).toBeGreaterThan(0);
		expect(result.results.length).toBeGreaterThan(0);

		// Should find report.txt
		const foundReport = result.results.some((r: any) => r.file.name === 'report');
		expect(foundReport).toBe(true);
	});

	test('should filter by file type across the library', async () => {
		console.log('[TS] Testing library-wide filter by .txt files...');

		const searchInput = {
			query: 'a', // Broad query
			scope: 'Library',
			mode: 'Normal',
			filters: {
				file_types: ['txt'],
			},
			sort: {
				field: 'Name',
				direction: 'Asc',
			},
			pagination: {
				limit: 50,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Filter result:', {
			total_found: result.total_found,
			results_count: result.results.length,
		});

		// All results should be .txt files
		result.results.forEach((r: any) => {
			expect(r.file.extension).toBe('txt');
		});
	});

	test('should search in a directory under the source', async () => {
		console.log('[TS] Testing path-scoped search in documents folder...');

		const documentsPath = `${bridgeConfig.source_path}/documents`;

		const searchInput = {
			query: 'notes',
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: bridgeConfig.device_slug,
							path: documentsPath,
						},
					},
				},
			},
			mode: 'Normal',
			filters: {},
			sort: {
				field: 'Relevance',
				direction: 'Desc',
			},
			pagination: {
				limit: 50,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Path search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
		});

		expect(result.results.length).toBeGreaterThan(0);

		// Should find notes.md
		const foundNotes = result.results.some((r: any) => r.file.name === 'notes');
		expect(foundNotes).toBe(true);
	});
});

describe('Search - Browsed Directory', () => {
	test('should search in a browsed directory', async () => {
		console.log('[TS] Testing browsed directory search for "video"...');

		const searchInput = {
			query: 'video',
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: bridgeConfig.device_slug,
							path: bridgeConfig.browsed_dir_path,
						},
					},
				},
			},
			mode: 'Normal',
			filters: {},
			sort: {
				field: 'Relevance',
				direction: 'Desc',
			},
			pagination: {
				limit: 50,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Browsed search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
		});

		// Debug: print all results
		if (result.results.length > 0) {
			console.log('[TS] Found files in browsed directory:');
			result.results.forEach((r: any, i: number) => {
				console.log(`  ${i + 1}. ${r.file.name} (score: ${r.score})`);
			});
		} else {
			console.log('[TS] ⚠️  NO RESULTS - This is the issue!');
		}

		// Assertions
		expect(result.total_found).toBeGreaterThan(0);
		expect(result.results.length).toBeGreaterThan(0);
	});

	test('should filter by file type in a browsed directory', async () => {
		console.log('[TS] Testing browsed directory filter by .mp3 files...');

		const searchInput = {
			query: 'a', // Broad query
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: bridgeConfig.device_slug,
							path: bridgeConfig.browsed_dir_path,
						},
					},
				},
			},
			mode: 'Normal',
			filters: {
				file_types: ['mp3'],
			},
			sort: {
				field: 'Name',
				direction: 'Asc',
			},
			pagination: {
				limit: 50,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Browsed filter result:', {
			total_found: result.total_found,
			results_count: result.results.length,
		});

		// All results should be .mp3 files
		result.results.forEach((r: any) => {
			expect(r.file.extension).toBe('mp3');
		});
	});

	test('should list all files in a browsed directory with broad query', async () => {
		console.log('[TS] Testing browsed directory broad search...');

		const searchInput = {
			query: 'a', // Very broad to catch most files
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: bridgeConfig.device_slug,
							path: bridgeConfig.browsed_dir_path,
						},
					},
				},
			},
			mode: 'Normal',
			filters: {},
			sort: {
				field: 'Name',
				direction: 'Asc',
			},
			pagination: {
				limit: 200,
				offset: 0,
			},
		};

		const result = await client.execute('query:search.files', searchInput);

		console.log('[TS] Broad search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
			files: result.results.map((r: any) => r.file.name),
		});

		// Should find at least some files (we created 4 files)
		expect(result.results.length).toBeGreaterThan(0);
	});
});
