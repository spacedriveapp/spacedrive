import type { File } from '@sd/ts-client';

export type MediaType = 'all' | 'images' | 'videos';

export function matchesMediaType(file: Pick<File, 'content_kind' | 'extension'>, type: MediaType): boolean {
	let kind = file.content_kind;
	if (kind === 'unknown' && file.extension) {
		const extension = file.extension.toLowerCase();
		if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'heic', 'heif', 'svg', 'bmp', 'tif', 'tiff', 'avif'].includes(extension)) kind = 'image';
		else if (['mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v', 'flv', 'wmv'].includes(extension)) kind = 'video';
	}
	return type === 'all' ? kind === 'image' || kind === 'video' : kind === (type === 'images' ? 'image' : 'video');
}

export function MediaTypeFilter({value, onChange}: {value: MediaType; onChange: (value: MediaType) => void}) {
	return (
		<div role="group" aria-label="Media type" className="flex gap-1 p-2 border-b border-app-line bg-app">
			{(['all', 'images', 'videos'] as const).map(type => (
				<button key={type} type="button" aria-pressed={value === type} onClick={() => onChange(type)} className={`px-3 py-1 rounded-md text-sm ${value === type ? 'bg-accent text-white' : 'text-ink-dull hover:bg-app-box hover:text-ink'}`}>
					{type === 'all' ? 'All' : type === 'images' ? 'Images' : 'Videos'}
				</button>
			))}
		</div>
	);
}
