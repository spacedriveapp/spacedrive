import dayjs from 'dayjs';

import { useBridgeMutation, useBridgeQuery, useLibraryMutation } from '@sd/client';
import { Button, Card } from '@sd/ui';
import { Database } from '~/components';
import { useLocale } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { Heading } from '../Layout';

// TODO: This is a non-library page but does a library query for backup. That will be confusing UX.
// TODO: Should this be a library or node page? If it's a library page how can a user view all their backups across libraries (say they wanted to save some storage cause their SSD is full)?
// TODO: If it were a library page what do we do when restoring a backup? It can't be a `useLibraryQuery` to restore cause we are gonna have to unload the library from the backend.

export const Component = () => {
	const platform = usePlatform();
	const { t } = useLocale();
	const backups = useBridgeQuery(['backups.getAll']);
	const doBackup = useLibraryMutation('backups.backup');
	const doRestore = useBridgeMutation('backups.restore');
	const doDelete = useBridgeMutation('backups.delete');

	return (
		<>
			<Heading
				title={t('backups')}
				description={t('backups_description')}
				rightArea={
					<div className="flex flex-row items-center space-x-5">
						<Button
							disabled={doBackup.isPending}
							variant="gray"
							size="md"
							onClick={() => {
								if (backups.data) {
									// TODO: opening paths from the frontend is hacky cause non-UTF-8 chars in the filename break stuff
									platform.openLink(backups.data.directory);
								}
							}}
						>
							Backups Directory
						</Button>
						<Button
							disabled={doBackup.isPending}
							variant="accent"
							size="md"
							onClick={() => doBackup.mutate(null)}
						>
							Backup
						</Button>
					</div>
				}
			/>

			{backups.data?.backups.map(backup => (
				<Card key={backup.id} className="hover:bg-app-box/70">
					<Database className="mr-3 size-10 self-center" />
					<div className="grid min-w-[110px] grid-cols-1">
						<h1 className="truncate pt-0.5 text-sm font-semibold">
							{dayjs(backup.timestamp).toString()}
						</h1>
						<p className="mt-0.5 select-text truncate text-sm text-ink-dull">
							{t('for_library', { name: backup.library_name })}
						</p>
					</div>
					<div className="flex grow" />
					<div className="flex h-[45px] space-x-2 p-2">
						<Button
							disabled={doRestore.isPending}
							onClick={() => doRestore.mutate(backup.path)}
							variant="gray"
						>
							{t('restore')}
						</Button>
						<Button
							disabled={doDelete.isPending}
							onClick={() => doDelete.mutate(backup.path)}
							size="sm"
							variant="colored"
							className="border-red-500 bg-red-500"
						>
							{t('delete')}
						</Button>
					</div>
				</Card>
			))}
		</>
	);
};
