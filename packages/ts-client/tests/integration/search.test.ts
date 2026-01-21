import { describe, test, expect, beforeAll } from 'bun:test';
import { readFile } from 'fs/promises';
import { SpacedriveClient } from '../../src/client';

interface SearchBridgeConfig {
	socket_addr: string;
	library_id: string;
	persistent_location_uuid: string;
	persistent_location_db_id: number;
	persistent_location_path: string;
	ephemeral_dir_path: string;
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
		persistent_path: bridgeConfig.persistent_location_path,
		ephemeral_path: bridgeConfig.ephemeral_dir_path,
	});

	// Connect to daemon via TCP socket
	client = SpacedriveClient.fromTcpSocket(bridgeConfig.socket_addr);
	client.setCurrentLibrary(bridgeConfig.library_id);

	console.log('[TS] Connected to daemon');
});

describe('Search - Persistent Location', () => {
	test('should search by query in persistent location', async () => {
		console.log('[TS] Testing persistent location search for "report"...');

		const searchInput = {
			query: 'report',
			scope: {
				Location: {
					location_id: bridgeConfig.persistent_location_uuid,
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

		console.log('[TS] Search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
			index_type: result.index_type,
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
		expect(result.index_type).toBe('Persistent');
		expect(result.total_found).toBeGreaterThan(0);
		expect(result.results.length).toBeGreaterThan(0);

		// Should find report.txt
		const foundReport = result.results.some((r: any) => r.file.name === 'report');
		expect(foundReport).toBe(true);
	});

	test('should filter by file type in persistent location', async () => {
		console.log('[TS] Testing persistent location filter by .txt files...');

		const searchInput = {
			query: 'a', // Broad query
			scope: {
				Location: {
					location_id: bridgeConfig.persistent_location_uuid,
				},
			},
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
			index_type: result.index_type,
		});

		expect(result.index_type).toBe('Persistent');

		// All results should be .txt files
		result.results.forEach((r: any) => {
			expect(r.file.extension).toBe('txt');
		});
	});

	test('should search in specific directory path', async () => {
		console.log('[TS] Testing path-scoped search in documents folder...');

		const documentsPath = `${bridgeConfig.persistent_location_path}/documents`;

		const searchInput = {
			query: 'notes',
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: await getDeviceSlug(),
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

		expect(result.index_type).toBe('Persistent');
		expect(result.results.length).toBeGreaterThan(0);

		// Should find notes.md
		const foundNotes = result.results.some((r: any) => r.file.name === 'notes');
		expect(foundNotes).toBe(true);
	});
});

describe('Search - Ephemeral Directory', () => {
	test('should search in ephemeral (non-indexed) directory', async () => {
		console.log('[TS] Testing ephemeral directory search for "video"...');

		const searchInput = {
			query: 'video',
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: await getDeviceSlug(),
							path: bridgeConfig.ephemeral_dir_path,
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

		console.log('[TS] Ephemeral search result:', {
			total_found: result.total_found,
			results_count: result.results.length,
			index_type: result.index_type,
		});

		// Debug: print all results
		if (result.results.length > 0) {
			console.log('[TS] Found files in ephemeral:');
			result.results.forEach((r: any, i: number) => {
				console.log(`  ${i + 1}. ${r.file.name} (score: ${r.score})`);
			});
		} else {
			console.log('[TS] ⚠️  NO RESULTS - This is the issue!');
		}

		// Assertions
		expect(result.index_type).toBe('Ephemeral');
		expect(result.total_found).toBeGreaterThan(0);
		expect(result.results.length).toBeGreaterThan(0);
	});

	test('should filter by file type in ephemeral directory', async () => {
		console.log('[TS] Testing ephemeral filter by .mp3 files...');

		const searchInput = {
			query: 'a', // Broad query
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: await getDeviceSlug(),
							path: bridgeConfig.ephemeral_dir_path,
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

		console.log('[TS] Ephemeral filter result:', {
			total_found: result.total_found,
			results_count: result.results.length,
		});

		expect(result.index_type).toBe('Ephemeral');

		// All results should be .mp3 files
		result.results.forEach((r: any) => {
			expect(r.file.extension).toBe('mp3');
		});
	});

	test('should list all files in ephemeral directory with broad query', async () => {
		console.log('[TS] Testing ephemeral broad search...');

		const searchInput = {
			query: 'a', // Very broad to catch most files
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: await getDeviceSlug(),
							path: bridgeConfig.ephemeral_dir_path,
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

		expect(result.index_type).toBe('Ephemeral');
		// Should find at least some files (we created 4 files)
		expect(result.results.length).toBeGreaterThan(0);
	});
});

describe('Search - Index Type Routing', () => {
	test('should correctly route to persistent index', async () => {
		const searchInput = {
			query: 'test',
			scope: {
				Location: {
					location_id: bridgeConfig.persistent_location_uuid,
				},
			},
			mode: 'Normal',
			filters: {},
			sort: { field: 'Relevance', direction: 'Desc' },
			pagination: { limit: 50, offset: 0 },
		};

		const result = await client.execute('query:search.files', searchInput);
		expect(result.index_type).toBe('Persistent');
	});

	test('should correctly route to ephemeral index', async () => {
		const searchInput = {
			query: 'test',
			scope: {
				Path: {
					path: {
						Physical: {
							device_slug: await getDeviceSlug(),
							path: bridgeConfig.ephemeral_dir_path,
						},
					},
				},
			},
			mode: 'Normal',
			filters: {},
			sort: { field: 'Relevance', direction: 'Desc' },
			pagination: { limit: 50, offset: 0 },
		};

		const result = await client.execute('query:search.files', searchInput);
		expect(result.index_type).toBe('Ephemeral');
	});
});

// Helper function to get device slug from the daemon
async function getDeviceSlug(): Promise<string> {
	// For now, use a hardcoded approach
	// TODO: This should come from the daemon API
	return 'james-s-macbook-pro'; // Matches the test environment
}
