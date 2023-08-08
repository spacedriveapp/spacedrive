import { useQuery } from '@tanstack/react-query';
import { Suspense } from 'react';
import { useLibraryContext } from '@sd/client';
import { ContextMenu } from '@sd/ui';
import { showAlertDialog } from '~/components';
import { Platform, usePlatform } from '~/util/Platform';
import { ConditionalItem } from '../ConditionalItem';
import { useContextMenuContext } from '../context';

export default new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths } = useContextMenuContext();
		const { getFilePathOpenWithApps, openFilePathWith } = usePlatform();

		if (!getFilePathOpenWithApps || !openFilePathWith) return null;
		if (selectedFilePaths.some((p) => p.is_dir)) return null;

		return { getFilePathOpenWithApps, openFilePathWith };
	},
	Component: ({ getFilePathOpenWithApps, openFilePathWith }) => (
		<ContextMenu.SubMenu label="Open with">
			<Suspense>
				<Items
					actions={{
						getFilePathOpenWithApps,
						openFilePathWith
					}}
				/>
			</Suspense>
		</ContextMenu.SubMenu>
	)
});

const Items = ({
	actions
}: {
	actions: Required<Pick<Platform, 'getFilePathOpenWithApps' | 'openFilePathWith'>>;
}) => {
	const { selectedFilePaths } = useContextMenuContext();

	const { library } = useLibraryContext();

	const ids = selectedFilePaths.map((obj) => obj.id);

	const items = useQuery<unknown>(
		['openWith', ids],
		() => actions.getFilePathOpenWithApps(library.uuid, ids),
		{ suspense: true }
	);

	return (
		<>
			{Array.isArray(items.data) && items.data.length > 0 ? (
				items.data.map((data, id) => (
					<ContextMenu.Item
						key={id}
						onClick={async () => {
							try {
								await actions.openFilePathWith(
									library.uuid,
									ids.map((id) => [id, data.url])
								);
							} catch (e) {
								console.error(e);
								showAlertDialog({
									title: 'Error',
									value: `Failed to open file, with: ${data.url}`
								});
							}
						}}
					>
						{data.name}
					</ContextMenu.Item>
				))
			) : (
				<p className="w-full text-center text-sm text-gray-400"> No apps available </p>
			)}
		</>
	);
};
