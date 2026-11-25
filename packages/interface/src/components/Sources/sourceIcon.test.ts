import DatabaseIcon from '@sd/assets/icons/Database.webp';
import DriveGoogleDriveIcon from '@sd/assets/icons/Drive-GoogleDrive.webp';
import DriveIcon from '@sd/assets/icons/Drive.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import ServerIcon from '@sd/assets/icons/Server.webp';
import {describe, expect, test} from 'bun:test';
import {sourceIcon} from './sourceIcon';

const filesystem = {data_type: 'filesystem', root: '/data'};

describe('sourceIcon', () => {
	test('draws each drive kind with its own icon', () => {
		expect(sourceIcon({...filesystem, volume_type: 'External'})).toBe(
			HDDIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Network'})).toBe(
			ServerIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Virtual'})).toBe(
			DatabaseIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Primary'})).toBe(
			DriveIcon
		);
	});

	test('names the cloud provider from the root', () => {
		expect(
			sourceIcon({
				data_type: 'filesystem',
				root: 'gdrive://photos',
				volume_type: 'Cloud'
			})
		).toBe(DriveGoogleDriveIcon);
	});

	test('falls back to the generic drive and the database icon', () => {
		expect(sourceIcon({...filesystem, volume_type: null})).toBe(DriveIcon);
		expect(
			sourceIcon({data_type: 'note', root: null, volume_type: null})
		).toBe(DatabaseIcon);
	});
});
