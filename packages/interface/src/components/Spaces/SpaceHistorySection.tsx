import {ArrowCounterClockwise, ClockCounterClockwise} from '@phosphor-icons/react';
import type {File, SpaceVersion} from '@sd/ts-client';
import {Button, toast} from '@spacedrive/primitives';
import {useState} from 'react';
import {useLibraryMutation, useLibraryQuery} from '../../contexts/SpacedriveContext';
import {formatBytes} from '../../routes/explorer/utils';
import {actorName, ago, useLocalPersonId, useSpaceScope} from '../Activity/ActivityFeed';
import {Section} from '../Inspector/Inspector';
import {useUndoDialog} from '../modals/UndoModal';

/** Who made a version, in the words the feed uses. */
function author(version: SpaceVersion, you: string | undefined) {
	if (version.origin === 'baseline') return 'When versioning started';
	if (version.origin === 'watcher') return 'Outside Spacedrive';
	const who = actorName(version.actor, you);
	return version.origin === 'restore' ? `${who} restored` : who;
}

/**
 * A file's versions in its space, newest first, each restorable. The newest
 * kept version is what is on disk, so it has no Restore.
 */
export function SpaceHistorySection({file}: {file: File}) {
	const path =
		file.kind !== 'Directory' && file.is_local && 'Physical' in file.sd_path
			? file.sd_path.Physical.path
			: '';
	const {scope} = useSpaceScope(path);
	const space = path && scope?.space.versioning ? scope.space : undefined;
	const you = useLocalPersonId();
	const openUndo = useUndoDialog();
	const history = useLibraryQuery(
		{
			type: 'spaces.history',
			input: {id: space?.id ?? '', path: scope?.relative ?? null, limit: 20}
		},
		{enabled: !!space, refetchInterval: 10_000}
	);
	const restore = useLibraryMutation('spaces.restore');
	const [lastRestore, setLastRestore] = useState<{undo: string; from: string; path: string} | null>(
		null
	);
	if (!space) return null;
	const versions = history.data?.versions ?? [];
	const name = scope!.relative.split('/').pop() ?? scope!.relative;

	const restoreVersion = async (version: SpaceVersion) => {
		try {
			const {restored} = await restore.mutateAsync({
				id: space.id,
				version: version.id,
				snapshot: null,
				path: null
			});
			void history.refetch();
			const landed = restored[0];
			if (!landed) {
				toast.info(`${name} already matches that version`);
				return;
			}
			setLastRestore(
				landed.undo
					? {undo: landed.undo, from: ago(version.at), path: scope!.relative}
					: null
			);
			toast.success(
				landed.conflict
					? `Restored to ${landed.landed}, since something took the name`
					: `Restored ${name} from ${ago(version.at)}`
			);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : String(error));
		}
	};

	return (
		<Section title="History" icon={ClockCounterClockwise}>
			{history.isLoading ? (
				<p className="text-sidebar-inkDull text-xs">Loading versions…</p>
			) : versions.length === 0 ? (
				<p className="text-sidebar-inkDull text-xs">
					No versions yet. {space.name} keeps one each time this file changes.
				</p>
			) : (
				<div className="space-y-0.5">
					{versions.map((version, index) => (
						<div
							key={version.id}
							className="hover:bg-app-box/40 flex items-center gap-2 rounded-md px-1 py-1 transition-colors"
						>
							<div className="min-w-0 flex-1">
								<div
									className="text-sidebar-ink truncate text-xs"
									title={new Date(version.at).toLocaleString()}
								>
									{author(version, you)}
								</div>
								<div className="text-sidebar-inkDull text-[11px]">
									{ago(version.at)} ·{' '}
									{version.deleted ? 'Deleted' : formatBytes(version.size)}
									{version.path !== scope!.relative && ` · ${version.path}`}
								</div>
							</div>
							{index === 0 && !version.deleted ? (
								<span className="text-sidebar-inkDull shrink-0 text-[11px]">Current</span>
							) : (
								!version.deleted && (
									<Button
										type="button"
										size="xs"
										variant="gray"
										className="shrink-0"
										disabled={restore.isPending}
										onClick={() => void restoreVersion(version)}
									>
										<ArrowCounterClockwise size={12} className="mr-1 inline" />
										Restore
									</Button>
								)
							)}
						</div>
					))}
				</div>
			)}
			{lastRestore?.path === scope!.relative && (
				<p className="text-sidebar-inkDull text-[11px]">
					Restored from {lastRestore.from}.{' '}
					<button
						type="button"
						className="text-accent hover:underline"
						onClick={() =>
							openUndo({
								job: lastRestore.undo,
								label: `the restore of ${name}`,
								onComplete: () => {
									setLastRestore(null);
									void history.refetch();
								}
							})
						}
					>
						Undo the restore
					</button>
				</p>
			)}
			<p className="text-sidebar-inkDull text-[11px]">
				Kept {history.data?.retention_days ?? '…'} days unless a snapshot pins them.
			</p>
		</Section>
	);
}
