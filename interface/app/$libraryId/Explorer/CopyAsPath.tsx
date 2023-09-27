import { ClipboardText } from '@phosphor-icons/react';
import { toast } from '@sd/ui';
import { Menu } from '~/components/Menu';

export const CopyAsPathBase = (
	props: { path: string } | { getPath: () => Promise<string | null> }
) => {
	return (
		<Menu.Item
			label="Copy as path"
			icon={ClipboardText}
			onClick={async () => {
				try {
					const path = 'path' in props ? props.path : await props.getPath();
					{
						/* 'path' in props
						? props.path
						: await libraryClient.query(['files.getPath', props.filePath.id]); */
					}

					if (path == null) throw new Error('No file path available');

					navigator.clipboard.writeText(path);
				} catch (error) {
					toast.error({
						title: `Failed to copy file path`,
						body: `Error: ${error}.`
					});
				}
			}}
		/>
	);
};
