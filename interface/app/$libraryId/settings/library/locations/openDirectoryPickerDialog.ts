import { Platform } from '~/util/Platform';

export const openDirectoryPickerDialog = async (platform: Platform): Promise<null | string> => {
	if (!platform.openDirectoryPickerDialog) return null;

	const path = await platform.openDirectoryPickerDialog();
	if (!path) return '';
	if (typeof path !== 'string')
		// TODO: Should adding multiple locations simultaneously be implemented?
		throw new Error('Adding multiple locations simultaneously is not supported');

	return path;
};
