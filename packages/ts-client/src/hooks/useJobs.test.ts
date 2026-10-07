import {describe, expect, test} from 'bun:test';
import {mergeJobSnapshots, type ExtendedJobListItem} from './useJobs';
import type {JobListItem} from '../generated/types';

const snapshot: JobListItem = {
	id: 'a', name: 'content_identity', device_id: 'device', status: 'running',
	progress: 0.2, action_type: null, action_context: null,
	created_at: '2026-10-08T00:00:00Z', started_at: '2026-10-08T00:00:00Z',
	completed_at: null, journal: null
};
const live: ExtendedJobListItem = {...snapshot, generic_progress: {
	percentage: 0.2, phase: 'Identifying', current_path: null, message: '200 files checked',
	completion: {completed: 200, total: 1000, bytes_completed: null, total_bytes: null},
	performance: {rate: 10, estimated_remaining: {secs: 80, nanos: 0}, elapsed: {secs: 20, nanos: 0}, error_count: 0, warning_count: 0}
}};

describe('job list refresh', () => {
	test('keeps runtime counts and ETA while accepting new running and paused snapshots', () => {
		for (const status of ['running', 'paused'] as const) {
			const [result] = mergeJobSnapshots([live], [{...snapshot, status, progress: 0.3}]);
			expect(result.status).toBe(status);
			expect(result.progress).toBe(0.3);
			expect(result.generic_progress).toBe(live.generic_progress);
		}
	});
	test('does not retain active progress for finished, new, or removed jobs', () => {
		const result = mergeJobSnapshots([live], [{...snapshot, status: 'completed', progress: 1}, {...snapshot, id: 'b'}]);
		expect(result.map(job => job.generic_progress)).toEqual([undefined, undefined]);
		expect(mergeJobSnapshots([live], [])).toEqual([]);
	});
});
