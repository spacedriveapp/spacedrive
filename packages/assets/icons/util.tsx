import * as icons from '.';

export const getIcon = (
	kind: string,
	isDir?: boolean,
	isDark?: boolean,
	extension?: string | null
) => {
	if (isDir) return icons[isDark ? 'Folder' : 'Folder_Light'];

	let document: Extract<keyof typeof icons, 'Document' | 'Document_Light'> = 'Document';
	if (extension) extension = `${kind}_${extension.toLowerCase()}`;
	if (!isDark) {
		kind = kind + '_Light';
		document = 'Document_Light';
		if (extension) extension = extension + '_Light';
	}

	return icons[
		(extension && extension in icons
			? extension
			: kind in icons
			? kind
			: document) as keyof typeof icons
	];
};
