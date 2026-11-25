import DatabaseIcon from '@sd/assets/icons/Database.webp';
import DriveLightIcon from '@sd/assets/icons/Drive_Light.webp';
import DriveGoogleDriveIcon from '@sd/assets/icons/Drive-GoogleDrive.webp';
import DriveIcon from '@sd/assets/icons/Drive.webp';
import HDDLightIcon from '@sd/assets/icons/HDD_Light.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import ServerLightIcon from '@sd/assets/icons/Server_Light.webp';
import ServerIcon from '@sd/assets/icons/Server.webp';
import {describe, expect, test} from 'bun:test';
import {sourceIcon} from './sourceIcon';

const filesystem = {data_type: 'filesystem', root: '/data', device_id: null};

describe('sourceIcon', () => {
	test('draws each drive kind with its own icon', () => {
		expect(sourceIcon({...filesystem, volume_type: 'External'}, true)).toBe(
			HDDIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Network'}, true)).toBe(
			ServerIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Virtual'}, true)).toBe(
			DatabaseIcon
		);
		expect(sourceIcon({...filesystem, volume_type: 'Primary'}, true)).toBe(
			DriveIcon
		);
	});

	test('uses the pale variants on dark themes', () => {
		expect(
			sourceIcon({...filesystem, volume_type: 'External'}, false)
		).toBe(HDDLightIcon);
		expect(sourceIcon({...filesystem, volume_type: null}, false)).toBe(
			DriveLightIcon
		);
	});

	test('names the cloud provider from the root', () => {
		expect(
			sourceIcon(
				{
					data_type: 'filesystem',
					root: 'gdrive://photos',
					volume_type: 'Cloud',
					device_id: null
				},
				true
			)
		).toBe(DriveGoogleDriveIcon);
	});

	test("draws a paired device's replica as a server", () => {
		const replica = {
			...filesystem,
			volume_type: null,
			device_id: '0198c7f2-6a1e-7c3b-9d2f-4e5a6b7c8d9e'
		};
		expect(sourceIcon(replica, true)).toBe(ServerIcon);
		expect(sourceIcon(replica, false)).toBe(ServerLightIcon);
	});

	test('falls back to the generic drive and the database icon', () => {
		expect(sourceIcon({...filesystem, volume_type: null}, true)).toBe(
			DriveIcon
		);
		expect(
			sourceIcon(
				{
					data_type: 'note',
					root: null,
					volume_type: null,
					device_id: null
				},
				true
			)
		).toBe(DatabaseIcon);
	});
});
