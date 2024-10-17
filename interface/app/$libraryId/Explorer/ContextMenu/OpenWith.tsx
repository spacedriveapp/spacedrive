import { useQuery } from '@tanstack/react-query';
import { Suspense } from 'react';

import { useLibraryContext } from '@sd/client';
import { toast } from '@sd/ui';
import { Menu } from '~/components/Menu';
import { useLocale } from '~/hooks';
import { OpenWithApplication, Platform, Result, usePlatform } from '~/util/Platform';

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
		if (selectedFilePaths.some(p => p.is_dir) || selectedEphemeralPaths.some(p => p.is_dir))
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

	const ids = selectedFilePaths.map(obj => obj.id);
	const paths = selectedEphemeralPaths.map(obj => obj.path);
	const { t } = useLocale();

	const { data: apps } = useQuery({
		queryKey: ['openWith', ids, paths],
		queryFn: async () => {
			const handleError = (res: Result<OpenWithApplication[], null>) => {
				if (res?.status === 'error') {
					toast.error('Failed to get applications capable to open file');
					if (res.error) console.error(res.error);
					return [];
				}
				return res?.data;
			};

			return Promise.all([
				ids.length > 0
					? actions.getFilePathOpenWithApps(library.uuid, ids).then(handleError)
					: Promise.resolve([]),
				paths.length > 0
					? actions.getEphemeralFilesOpenWithApps(paths).then(handleError)
					: Promise.resolve([])
			])
				.then(res => res.flat())
				.then(res => res.sort((a, b) => a.name.localeCompare(b.name)));
		},
		initialData: []
	});

	return (
		<>
			{apps.length > 0 ? (
				apps.map((data, index) => (
					<Menu.Item
						key={index}
						onClick={async () => {
							try {
								if (ids.length > 0) {
									await actions.openFilePathWith(
										library.uuid,
										ids.map(id => [id, data.url])
									);
								}

								if (paths.length > 0) {
									await actions.openEphemeralFileWith(
										paths.map(path => [path, data.url])
									);
								}
							} catch (e) {
								toast.error(t('failed_to_open_file_with', { data: data.url }));
							}
						}}
					>
						{data.name}
					</Menu.Item>
				))
			) : (
				<p className="w-full text-center text-sm text-gray-400">{t('no_apps_available')}</p>
			)}
		</>
	);
};
