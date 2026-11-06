import {describe, expect, test} from 'bun:test';
import {createElement} from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import type {JobListItem} from '../types';
import {JobProgressSummary} from './JobProgressSummary';

const job: JobListItem = {
	id: 'fixture',
	name: 'file_copy',
	device_id: 'fixture',
	status: 'running',
	progress: 0.2,
	action_type: null,
	action_context: null,
	created_at: '2026-10-13T00:00:00Z',
	started_at: null,
	completed_at: null,
	journal: null,
	generic_progress: {
		percentage: 0.2,
		phase: 'Processing',
		current_path: null,
		message: 'Copying items',
		completion: {
			completed: 200,
			total: 1000,
			bytes_completed: null,
			total_bytes: null
		},
		performance: {
			rate: 10,
			estimated_remaining: null,
			elapsed: {secs: 20, nanos: 0},
			error_count: 0,
			warning_count: 0
		}
	}
};
describe('generic job summary', () => {
	test('known counts and rate work without a content identity job name', () => {
		const markup = renderToStaticMarkup(
			createElement(JobProgressSummary, {job})
		);
		expect(markup).toContain('200 / 1,000');
		expect(markup).toContain('items/s');
	});
	test('unknown totals retain real completed counts for any job', () => {
		const markup = renderToStaticMarkup(
			createElement(JobProgressSummary, {
				job: {
					...job,
					generic_progress: {
						...job.generic_progress!,
						completion: {
							...job.generic_progress!.completion,
							total: 0
						}
					}
				}
			})
		);
		expect(markup).toContain('200 processed');
		expect(markup).toContain('Estimating');
	});
	test('a running job without a report cannot display 100 percent', () => {
		const markup = renderToStaticMarkup(
			createElement(JobProgressSummary, {
				job: {...job, progress: 1, generic_progress: undefined}
			})
		);
		expect(markup).toContain('99%');
		expect(markup).not.toContain('100%');
	});
});
