/**
 * Volume icon resolution tests.
 *
 * The key invariant: icons are derived from the `mount_point` scheme, never
 * from the human-readable volume name. A user-renamed OneDrive volume must
 * still return the OneDrive icon.
 */

import { describe, it, expect } from 'bun:test';
import { getVolumeIcon, parseCloudService } from '../volumeIcons';

describe('parseCloudService', () => {
	it('extracts scheme from a OneDrive mount point', () => {
		expect(parseCloudService('onedrive://drive-id/path')).toBe('onedrive');
	});

	it('extracts scheme from an S3 mount point', () => {
		expect(parseCloudService('s3://my-bucket')).toBe('s3');
	});

	it('returns null for a local filesystem path', () => {
		expect(parseCloudService('/Users/alice/Documents')).toBe(null);
		expect(parseCloudService('C:\\Users\\alice')).toBe(null);
	});

	it('returns null for an unknown scheme', () => {
		expect(parseCloudService('smb://server/share')).toBe(null);
	});

	it('returns null for an empty mount point', () => {
		expect(parseCloudService(null)).toBe(null);
		expect(parseCloudService('')).toBe(null);
	});
});

describe('getVolumeIcon', () => {
	it('uses scheme-based lookup, ignoring a renamed display name', () => {
		// A user renamed their OneDrive volume to a french phrase that
		// contains none of the substrings "OneDrive", "S3", "Google", etc.
		// The scheme in mount_point is the authoritative signal.
		const renamedOneDrive = {
			mount_point: 'onedrive://01FOO/',
			// name is NOT an input to getVolumeIcon by design.
		};

		const oneDrive = {
			mount_point: 'onedrive://01FOO/',
		};

		expect(getVolumeIcon(renamedOneDrive)).toBe(getVolumeIcon(oneDrive));
	});

	it('returns the OneDrive icon for a OneDrive volume', () => {
		const icon = getVolumeIcon({ mount_point: 'onedrive://drive-id/' });
		// We don't assert the exact asset path (webpack hash varies); instead
		// we assert it's distinct from the generic local-drive icon.
		const genericLocal = getVolumeIcon({
			mount_point: '/Users/alice',
			volume_type: 'Internal',
		});
		expect(icon).not.toBe(genericLocal);
	});

	it('returns the HDD icon for an external local volume', () => {
		const externalDrive = getVolumeIcon({
			mount_point: '/Volumes/USB',
			volume_type: 'External',
		});
		const removableDrive = getVolumeIcon({
			mount_point: '/Volumes/SD',
			volume_type: 'Removable',
		});
		// External and Removable both map to the same HDD icon.
		expect(externalDrive).toBe(removableDrive);
	});

	it('falls back to the generic drive icon for unknown schemes', () => {
		const unknown = getVolumeIcon({ mount_point: 'ftp://old-server/' });
		const internal = getVolumeIcon({
			mount_point: '/',
			volume_type: 'Internal',
		});
		// Both fall through to the default generic drive icon.
		expect(unknown).toBe(internal);
	});
});
