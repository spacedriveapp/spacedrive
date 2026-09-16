import {describe, expect, test} from 'bun:test';
import {normalizeAssetProtocolPath} from '../src/lib/assetPath';

describe('normalizeAssetProtocolPath', () => {
	test('maps a canonical macOS home path to the user-facing alias', () => {
		expect(
			normalizeAssetProtocolPath(
				'/System/Volumes/Data/Users/james/Desktop/photo.png'
			)
		).toBe('/Users/james/Desktop/photo.png');
	});

	test('maps a canonical macOS external-volume path', () => {
		expect(
			normalizeAssetProtocolPath(
				'/System/Volumes/Data/Volumes/Archive/video.mov'
			)
		).toBe('/Volumes/Archive/video.mov');
	});

	test('leaves other paths unchanged', () => {
		expect(normalizeAssetProtocolPath('/tmp/preview.txt')).toBe(
			'/tmp/preview.txt'
		);
		expect(
			normalizeAssetProtocolPath('C:\\Users\\james\\preview.txt')
		).toBe('C:\\Users\\james\\preview.txt');
	});
});
