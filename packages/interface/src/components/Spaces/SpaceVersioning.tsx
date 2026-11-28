import {ArrowCounterClockwise, Camera, Trash} from '@phosphor-icons/react';
import type {Space, SpaceSnapshot} from '@sd/ts-client';
import {Button, Input, Switch, toast} from '@spacedrive/primitives';
import {useState} from 'react';
import {useLibraryMutation, useLibraryQuery} from '../../contexts/SpacedriveContext';
import {formatBytes} from '../../routes/explorer/utils';
import {actorName, ago, useLocalPersonId} from '../Activity/ActivityFeed';

function message(error: unknown) {
	return error instanceof Error ? error.message : String(error);
}

/**
 * Versioning for a space: the toggle the settings form saves, and the kept
 * history, which outlives the toggle, with its snapshots and purge.
 */
export function SpaceVersioning({
	space,
	checked,
	onChange
}: {
	space: Space;
	checked: boolean;
	onChange: (checked: boolean) => void;
}) {
	const history = useLibraryQuery({
		type: 'spaces.history',
		input: {id: space.id, path: null, limit: 1}
	});
	const snapshots = useLibraryQuery({type: 'spaces.snapshot.list', input: {id: space.id}});
	const kept =
		space.versioning ||
		(history.data?.versions.length ?? 0) > 0 ||
		(snapshots.data?.snapshots.length ?? 0) > 0;

	return (
		<div className="bg-app-box space-y-3 rounded-md px-3 py-2">
			<label className="flex cursor-pointer items-center justify-between gap-3">
				<div>
					<div className="text-ink text-sm">Versioning</div>
					<div className="text-ink-dull text-[11px]">
						Keep the bytes of every change, so you can restore a file
						or the whole space.
					</div>
				</div>
				<Switch checked={checked} onCheckedChange={onChange} size="sm" />
			</label>
			{space.versioning && !checked && (
				<div className="text-ink-dull text-[11px]">
					Turning versioning off stops new versions. The history and
					snapshots already kept stay until they age out or you purge
					them.
				</div>
			)}
			{!space.versioning && checked && (
				<div className="text-ink-dull text-[11px]">
					Spacedrive captures the files already here in the background.
				</div>
			)}
			{kept && history.data && (
				<>
					<div className="text-ink-dull text-[11px]">
						Versions are kept {history.data.retention_days} days unless a
						snapshot pins them.
					</div>
					<Snapshots
						space={space}
						snapshots={snapshots.data?.snapshots ?? []}
						refetch={() => void snapshots.refetch()}
					/>
					<Purge
						space={space}
						onPurged={() => {
							void history.refetch();
							void snapshots.refetch();
						}}
					/>
				</>
			)}
		</div>
	);
}

function Snapshots({
	space,
	snapshots,
	refetch
}: {
	space: Space;
	snapshots: SpaceSnapshot[];
	refetch: () => void;
}) {
	const you = useLocalPersonId();
	const [name, setName] = useState('');
	const [confirming, setConfirming] = useState<string | null>(null);
	const create = useLibraryMutation('spaces.snapshot.create', {onSuccess: refetch});
	const remove = useLibraryMutation('spaces.snapshot.delete', {onSuccess: refetch});
	const restore = useLibraryMutation('spaces.restore');

	const take = async () => {
		if (!name.trim()) return;
		try {
			await create.mutateAsync({id: space.id, name: name.trim()});
			setName('');
		} catch (error) {
			toast.error(message(error));
		}
	};

	const restoreAll = async (snapshot: SpaceSnapshot) => {
		setConfirming(null);
		try {
			const result = await restore.mutateAsync({
				id: space.id,
				version: null,
				snapshot: snapshot.id,
				path: null
			});
			const failed = result.failed.length ? `, ${result.failed.length} failed` : '';
			toast.success(
				`Restored ${result.restored.length} files from ${snapshot.name}, ${result.unchanged} already matched${failed}`
			);
		} catch (error) {
			toast.error(message(error));
		}
	};

	const removeSnapshot = async (snapshot: SpaceSnapshot) => {
		try {
			await remove.mutateAsync({id: space.id, snapshot: snapshot.id});
		} catch (error) {
			toast.error(message(error));
		}
	};

	return (
		<div>
			<div className="text-ink text-xs font-medium">Snapshots</div>
			<div className="text-ink-dull text-[11px]">
				A named state of the whole space. Its versions are kept until you
				delete it.
			</div>
			<div className="mt-1.5 space-y-1">
				{snapshots.map((snapshot) => (
					<div
						key={snapshot.id}
						className="bg-app-darkBox flex items-center gap-2 rounded px-2 py-1.5"
					>
						<div className="min-w-0 flex-1">
							<div className="text-ink truncate text-xs">{snapshot.name}</div>
							<div className="text-ink-dull text-[11px]">
								{snapshot.files} files · {formatBytes(snapshot.bytes)} ·{' '}
								{actorName(snapshot.created_by, you)} · {ago(snapshot.at)}
							</div>
						</div>
						{confirming === snapshot.id ? (
							<>
								<span className="text-ink-dull text-[11px]">Each file can be undone.</span>
								<Button
									type="button"
									size="xs"
									variant="accent"
									disabled={restore.isPending}
									onClick={() => void restoreAll(snapshot)}
								>
									Restore all
								</Button>
							</>
						) : (
							<Button
								type="button"
								size="xs"
								variant="gray"
								disabled={restore.isPending}
								onClick={() => setConfirming(snapshot.id)}
							>
								<ArrowCounterClockwise size={12} className="mr-1 inline" />
								Restore all…
							</Button>
						)}
						<Button
							type="button"
							size="xs"
							variant="gray"
							title="Delete snapshot"
							disabled={remove.isPending}
							onClick={() => void removeSnapshot(snapshot)}
						>
							<Trash size={12} />
						</Button>
					</div>
				))}
			</div>
			{space.versioning && (
				<div className="mt-1.5 flex items-center gap-2">
					<Input
						size="sm"
						className="flex-1"
						value={name}
						placeholder="Snapshot name, e.g. Before cleanup"
						onChange={(event) => setName(event.target.value)}
						onKeyDown={(event) => {
							if (event.key !== 'Enter') return;
							event.preventDefault();
							void take();
						}}
					/>
					<Button
						type="button"
						size="sm"
						variant="gray"
						disabled={!name.trim() || create.isPending}
						onClick={() => void take()}
					>
						<Camera size={14} weight="bold" className="mr-1.5 inline" />
						Take snapshot
					</Button>
				</div>
			)}
		</div>
	);
}

function Purge({space, onPurged}: {space: Space; onPurged: () => void}) {
	const [confirming, setConfirming] = useState(false);
	const purge = useLibraryMutation('spaces.history.purge');

	const run = async () => {
		try {
			await purge.mutateAsync({id: space.id});
			setConfirming(false);
			onPurged();
			toast.success(`Deleted the history of ${space.name}`);
		} catch (error) {
			toast.error(message(error));
		}
	};

	if (!confirming) {
		return (
			<Button type="button" size="sm" variant="gray" onClick={() => setConfirming(true)}>
				<Trash size={14} weight="bold" className="mr-1.5 inline" />
				Purge history…
			</Button>
		);
	}
	return (
		<div className="flex items-center justify-between gap-2">
			<span className="text-ink-dull text-[11px]">
				Deletes every version and snapshot of {space.name}. This cannot be
				undone.
			</span>
			<div className="flex shrink-0 gap-1.5">
				<Button type="button" size="sm" variant="gray" onClick={() => setConfirming(false)}>
					Cancel
				</Button>
				<Button
					type="button"
					size="sm"
					variant="colored"
					className="bg-status-error text-white"
					disabled={purge.isPending}
					onClick={() => void run()}
				>
					Purge history
				</Button>
			</div>
		</div>
	);
}
