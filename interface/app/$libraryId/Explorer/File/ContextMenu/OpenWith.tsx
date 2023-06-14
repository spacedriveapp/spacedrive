import { useQuery } from '@tanstack/react-query';
import { Suspense } from 'react';
import { FilePath, useLibraryContext } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { Platform, usePlatform } from '~/util/Platform';

export default (props: { filePath: FilePath }) => {
	const { getFilePathOpenWithApps, openFilePathWith } = usePlatform();

	if (!getFilePathOpenWithApps || !openFilePathWith) return null;
	if (props.filePath.is_dir) return null;
	return (
		<ContextMenu.SubMenu label="Open with">
			<Suspense>
				<Items
					filePath={props.filePath}
					actions={{
						getFilePathOpenWithApps,
						openFilePathWith
					}}
				/>
			</Suspense>
		</ContextMenu.SubMenu>
	);
};

const Items = ({
	filePath,
	actions
}: {
	filePath: FilePath;
	actions: Required<Pick<Platform, 'getFilePathOpenWithApps' | 'openFilePathWith'>>;
}) => {
	const { library } = useLibraryContext();

	const items = useQuery<any[]>(
		['openWith', filePath.id],
		() => actions.getFilePathOpenWithApps(library.uuid, [filePath.id]),
		{ suspense: true }
	);

	return (
		<>
			{items.data?.map((data) => (
				<ContextMenu.Item
					key={data.name}
					onClick={async () => {
						try {
							await actions.openFilePathWith(library.uuid, [(filePath.id, data.c.url)]);
						} catch {
							showAlertDialog({
								title: 'Error',
								value: `Failed to open file, with: ${data.url}`
							});
						}
					}}
				>
					{data.name}
				</ContextMenu.Item>
			)) ?? <p> No apps available </p>}
		</>
	);
};
