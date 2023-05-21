import * as icons from '../icons';

// Record is defined as follows inside TypeScript
export type IconTypes<K = keyof typeof icons> = K extends `${string}_${string}` ? never : K;

export const iconNames = Object.fromEntries(
	Object.keys(icons)
		.filter((key) => !key.includes('_'))
		.map((key) => [key, key])
) as Record<IconTypes, string>;

export const getIcon = (
	kind: string,
	isDark?: boolean,
	extension?: string | null,
	isDir?: boolean
) => {
	if (isDir) return icons[isDark ? 'Folder' : 'Folder_Light'];

	let document: Extract<keyof typeof icons, 'Document' | 'Document_Light'> = 'Document';
	if (extension) extension = `${kind}_${extension.toLowerCase()}`;
	if (!isDark) {
		document = 'Document_Light';
		if (extension) extension = extension + '_Light';
	}

	const lightKind = kind + '_Light';
	return icons[
		(extension && extension in icons
			? extension
			: !isDark && lightKind in icons
			? lightKind
			: kind in icons
			? kind
			: document) as keyof typeof icons
	];
};
