import { useQuery } from '@tanstack/react-query';
import { Suspense } from 'react';
import { useLibraryContext } from '@sd/client';
import { toast } from '@sd/ui';
import { Menu } from '~/components/Menu';
import { useLocale } from '~/hooks';
import { Platform, usePlatform } from '~/util/Platform';

import { ConditionalItem } from './ConditionalItem';
import { useContextMenuContext } from './context';

export default new ConditionalItem({
	useCondition: () => {
		const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();
		const {
			getFilePathOpenWithApps,
			openFilePathWith,
			getEphemeralFilesOpenWithApps,
			openEphemeralFileWith
		} = usePlatform();

		if (
			!getFilePathOpenWithApps ||
			!openFilePathWith ||
			!getEphemeralFilesOpenWithApps ||
			!openEphemeralFileWith
		)
			return null;
		if (selectedFilePaths.some((p) => p.is_dir) || selectedEphemeralPaths.some((p) => p.is_dir))
			return null;

		return {
			getFilePathOpenWithApps,
			openFilePathWith,
			getEphemeralFilesOpenWithApps,
			openEphemeralFileWith
		};
	},
	Component: ({
		getFilePathOpenWithApps,
		openFilePathWith,
		getEphemeralFilesOpenWithApps,
		openEphemeralFileWith
	}) => {
		const { t } = useLocale();
		return (
			<Menu.SubMenu label={t('open_with')}>
				<Suspense>
					<Items
						actions={{
							getFilePathOpenWithApps,
							openFilePathWith,
							getEphemeralFilesOpenWithApps,
							openEphemeralFileWith
						}}
					/>
				</Suspense>
			</Menu.SubMenu>
		);
	}
});

const Items = ({
	actions
}: {
	actions: Required<
		Pick<
			Platform,
			| 'getFilePathOpenWithApps'
			| 'openFilePathWith'
			| 'getEphemeralFilesOpenWithApps'
			| 'openEphemeralFileWith'
		>
	>;
}) => {
	const { selectedFilePaths, selectedEphemeralPaths } = useContextMenuContext();

	const { library } = useLibraryContext();

	const ids = selectedFilePaths.map((obj) => obj.id);
	const paths = selectedEphemeralPaths.map((obj) => obj.path);

	const items = useQuery<unknown>(
		['openWith', ids, paths],
		() => {
			if (ids.length > 0) return actions.getFilePathOpenWithApps(library.uuid, ids);
			else if (paths.length > 0) return actions.getEphemeralFilesOpenWithApps(paths);
			else return { data: [] };
		},
		{ suspense: true }
	);

	return (
		<>
			{Array.isArray(items.data) && items.data.length > 0 ? (
				items.data.map((data, index) => (
					<Menu.Item
						key={index}
						onClick={async () => {
							try {
								if (ids.length > 0) {
									await actions.openFilePathWith(
										library.uuid,
										ids.map((id) => [id, data.url])
									);
								} else if (paths.length > 0) {
									await actions.openEphemeralFileWith(
										paths.map((path) => [path, data.url])
									);
								}
							} catch (e) {
								toast.error(`Failed to open file, with: ${data.url}`);
							}
						}}
					>
						{data.name}
					</Menu.Item>
				))
			) : (
				<p className="w-full text-center text-sm text-gray-400"> No apps available </p>
			)}
		</>
	);
};
