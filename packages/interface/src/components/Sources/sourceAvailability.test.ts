import {describe, expect, test} from 'bun:test';
import {
	availabilityBadge,
	availabilityDescription,
	sourceAvailability
} from './sourceAvailability';

describe('sourceAvailability', () => {
	test('the volume state outranks the attached flag', () => {
		expect(
			sourceAvailability({attached: false, volume_state: 'Locked'})
		).toBe('locked');
		expect(
			sourceAvailability({attached: false, volume_state: 'Unmounted'})
		).toBe('unmounted');
		expect(
			sourceAvailability({attached: true, volume_state: 'Mounted'})
		).toBe('available');
	});

	test('without a volume state, detached means offline', () => {
		expect(sourceAvailability({attached: false, volume_state: null})).toBe(
			'offline'
		);
		expect(sourceAvailability({attached: false})).toBe('offline');
		expect(sourceAvailability({attached: true, volume_state: null})).toBe(
			'available'
		);
	});

	test('a badge names every state but available', () => {
		expect(availabilityBadge('locked')).toBe('Locked');
		expect(availabilityBadge('unmounted')).toBe('Unmounted');
		expect(availabilityBadge('offline')).toBe('Offline');
		expect(availabilityBadge('available')).toBeUndefined();
	});

	test('the description names the volume and says the index stays', () => {
		expect(availabilityDescription('locked', 'vault')).toBe(
			'vault is locked: its encryption key is not loaded. Files stay listed from the last index and cannot be opened until the key is loaded.'
		);
		expect(availabilityDescription('unmounted', undefined)).toBe(
			'Its volume is not mounted. Files stay listed from the last index and cannot be opened until it mounts.'
		);
		expect(availabilityDescription('offline', 'vault')).toContain(
			'not connected'
		);
		expect(availabilityDescription('available', 'vault')).toBeUndefined();
	});
});
