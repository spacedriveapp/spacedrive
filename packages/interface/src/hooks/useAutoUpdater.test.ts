import {describe, expect, test} from 'bun:test';
import type {AvailableUpdate} from '../contexts/PlatformContext';
import {watchForUpdates} from './useAutoUpdater';

const update = (version: string): AvailableUpdate => ({
	version,
	currentVersion: '2.0.0-beta.0',
	installAndRestart: async () => {}
});

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('watchForUpdates', () => {
	test('checks at once and then on every interval', async () => {
		let checks = 0;
		const stop = watchForUpdates({
			check: async () => {
				checks++;
				return null;
			},
			onUpdate: () => {},
			intervalMs: 20
		});
		expect(checks).toBe(1);
		await sleep(70);
		stop();
		expect(checks).toBeGreaterThanOrEqual(3);
		const after = checks;
		await sleep(50);
		expect(checks).toBe(after);
	});

	test('prompts once per version', async () => {
		const versions = ['2.0.0-beta.1', '2.0.0-beta.1', '2.0.0-beta.2'];
		const prompted: string[] = [];
		const stop = watchForUpdates({
			check: async () => update(versions.shift() ?? '2.0.0-beta.2'),
			onUpdate: (found) => prompted.push(found.version),
			intervalMs: 10
		});
		await sleep(60);
		stop();
		expect(prompted).toEqual(['2.0.0-beta.1', '2.0.0-beta.2']);
	});

	test('a failed check stays silent and the next one still runs', async () => {
		let checks = 0;
		const prompted: string[] = [];
		const stop = watchForUpdates({
			check: async () => {
				checks++;
				if (checks === 1) throw new Error('offline');
				return update('2.0.0-beta.1');
			},
			onUpdate: (found) => prompted.push(found.version),
			intervalMs: 10
		});
		await sleep(40);
		stop();
		expect(prompted).toEqual(['2.0.0-beta.1']);
	});

	test('a check that finishes after stop does not prompt', async () => {
		const prompted: string[] = [];
		const stop = watchForUpdates({
			check: async () => {
				await sleep(20);
				return update('2.0.0-beta.1');
			},
			onUpdate: (found) => prompted.push(found.version),
			intervalMs: 1000
		});
		stop();
		await sleep(40);
		expect(prompted).toEqual([]);
	});
});
