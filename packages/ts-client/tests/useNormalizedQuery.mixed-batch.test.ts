import {describe, expect, test} from 'bun:test';
import {QueryClient} from '@tanstack/react-query';
import {
	filterBatchResources,
	updateBatchResources,
	type UseNormalizedQueryOptions
} from '../src/hooks/useNormalizedQuery';

const queryKey = ['query:files.directory_listing', 'library-id', {}];

function file(id: string, path: string, name = id) {
	return {
		id,
		name,
		sd_path: {
			Physical: {
				device_slug: 'device',
				path
			}
		}
	};
}

function options(path: string): UseNormalizedQueryOptions<any> {
	return {
		query: 'files.directory_listing',
		resourceType: 'file',
		pathScope: {
			Physical: {
				device_slug: 'device',
				path
			}
		},
		includeDescendants: false
	};
}

function updateCache(
	initialFiles: ReturnType<typeof file>[],
	resources: ReturnType<typeof file>[],
	pathScope: string
) {
	const queryClient = new QueryClient();
	queryClient.setQueryData(queryKey, {
		files: initialFiles,
		total_count: initialFiles.length,
		has_more: false
	});

	updateBatchResources(
		resources,
		null,
		options(pathScope),
		queryKey,
		queryClient
	);

	return queryClient.getQueryData(queryKey) as {
		files: ReturnType<typeof file>[];
		total_count: number;
		has_more: boolean;
	};
}

describe('updateBatchResources scoped batches', () => {
	test('removes out-of-scope IDs and merges in-scope resources in one mixed batch', () => {
		const result = updateCache(
			[
				file('moved-out', 'C:\\Users\\Test\\Current\\moved.txt'),
				file(
					'staying',
					'C:\\Users\\Test\\Current\\staying.txt',
					'old-name'
				),
				file('untouched', 'C:\\Users\\Test\\Current\\untouched.txt')
			],
			[
				file('moved-out', 'C:/Users/Test/Other/moved.txt'),
				file(
					'staying',
					'c:/users/test/current/staying.txt',
					'new-name'
				),
				file('moved-in', 'C:/USERS/TEST/CURRENT/moved-in.txt')
			],
			'C:\\Users\\Test\\Current\\'
		);

		expect(result.files.map(({id}) => id)).toEqual([
			'staying',
			'untouched',
			'moved-in'
		]);
		expect(result.files.find(({id}) => id === 'staying')?.name).toBe(
			'new-name'
		);
	});

	test('applies the same mixed-batch update to direct array caches', () => {
		const queryClient = new QueryClient();
		queryClient.setQueryData(queryKey, [
			file('moved-out', '/current/moved.txt'),
			file('staying', '/current/staying.txt', 'old-name')
		]);

		updateBatchResources(
			[
				file('moved-out', '/other/moved.txt'),
				file('staying', '/current/staying.txt', 'new-name')
			],
			null,
			options('/current'),
			queryKey,
			queryClient
		);

		expect(queryClient.getQueryData(queryKey)).toEqual([
			file('staying', '/current/staying.txt', 'new-name')
		]);
	});

	test('removes every cached resource when the entire batch moves out of scope', () => {
		const result = updateCache(
			[
				file('first', '/current/first.txt'),
				file('second', '/current/second.txt')
			],
			[
				file('first', '/other/first.txt'),
				file('second', '/other/second.txt')
			],
			'/current'
		);

		expect(result.files).toEqual([]);
	});

	test('merges every resource when the entire batch remains in scope', () => {
		const result = updateCache(
			[file('existing', '/current/existing.txt', 'old-name')],
			[
				file('existing', '/current/existing.txt', 'new-name'),
				file('new', '/current/new.txt')
			],
			'/current/'
		);

		expect(result.files.map(({id}) => id)).toEqual(['existing', 'new']);
		expect(result.files[0]?.name).toBe('new-name');
	});

	test('keeps POSIX path matching case-sensitive', () => {
		const resources = [
			file('matching', '/Users/Test/Current/file.txt'),
			file('different-case', '/users/test/current/file.txt')
		];

		expect(
			filterBatchResources(resources, options('/Users/Test/Current'))
		).toEqual([resources[0]]);
	});
});
