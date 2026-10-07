import type {File} from '@sd/ts-client';
import {describe, expect, test} from 'bun:test';
import {createCopyMenuItems} from './copyMenu';

const file = {
	name: 'صورة one.PNG',
	extension: 'PNG',
	kind: 'File',
	is_local: true,
	sd_path: {
		Physical: {device_slug: 'test', path: '/Test folder/صورة one.PNG'}
	}
} satisfies Pick<File, 'name' | 'extension' | 'kind' | 'is_local' | 'sd_path'>;
function setup(
	targets: Parameters<typeof createCopyMenuItems>[0] = [file],
	fail = false
) {
	const writes: string[] = [];
	let cleared = 0;
	let files = 0;
	let errors = 0;
	const menu = createCopyMenuItems(targets, {
		copyFiles: () => {
			files++;
		},
		writeText: async (text) => {
			if (fail) throw Error('denied');
			writes.push(text);
		},
		writeImage: async (path) => {
			writes.push(`image:${path}`);
		},
		clearFiles: () => {
			cleared++;
		},
		onError: () => {
			errors++;
		}
	});
	return {menu, writes, state: () => ({cleared, files, errors})};
}
describe('Copy choices', () => {
	test('name retains extension, spaces and Arabic; pathname stays exact', async () => {
		const s = setup();
		await s.menu.find((i) => i.label === 'Copy Name')!.onClick!();
		await s.menu.find((i) => i.label === 'Copy Pathname')!.onClick!();
		expect(s.writes).toEqual([file.name, file.sd_path.Physical.path]);
		expect(s.state().cleared).toBe(2);
	});
	test('multiple names and paths preserve selection order with newline separators', async () => {
		const s = setup([
			file,
			{
				...file,
				name: 'two.PNG',
				sd_path: {Physical: {device_slug: 'test', path: '/two.PNG'}}
			}
		]);
		await s.menu.find((i) => i.label === 'Copy Names')!.onClick!();
		await s.menu.find((i) => i.label === 'Copy Pathnames')!.onClick!();
		expect(s.writes).toEqual([
			`${file.name}\ntwo.PNG`,
			`${file.sd_path.Physical.path}\n/two.PNG`
		]);
		expect(s.menu.some((i) => i.label === 'Copy Image')).toBe(false);
	});
	test('image writes local image content via platform callback', async () => {
		const s = setup();
		await s.menu.find((i) => i.label === 'Copy Image')!.onClick!();
		expect(s.writes).toEqual([`image:${file.sd_path.Physical.path}`]);
		expect(s.state().cleared).toBe(1);
	});
	test('remote, directory and unsupported format do not expose image copy', () => {
		for (const target of [
			{...file, is_local: false},
			{...file, kind: 'Directory' as const},
			{...file, extension: 'heic'}
		])
			expect(
				setup([target]).menu.some((i) => i.label === 'Copy Image')
			).toBe(false);
	});
	test('a mixed physical and content selection disables pathname copy instead of copying a partial list', () => {
		const s = setup([
			file,
			{...file, sd_path: {Content: {content_id: 'test'}}}
		]);
		expect(s.menu.find((i) => i.label === 'Copy Pathnames')!.disabled).toBe(
			true
		);
	});
	test('failed clipboard write keeps pending file clipboard and reports failure', async () => {
		const s = setup([file], true);
		await s.menu.find((i) => i.label === 'Copy Name')!.onClick!();
		expect(s.state()).toEqual({cleared: 0, files: 0, errors: 1});
	});
	test('file copy keeps the original operation and shortcut', () => {
		const s = setup();
		const item = s.menu.find((i) => i.label === 'Copy File')!;
		item.onClick!();
		expect(item.keybindId).toBe('explorer.copy');
		expect(s.state()).toEqual({cleared: 0, files: 1, errors: 0});
	});
});
