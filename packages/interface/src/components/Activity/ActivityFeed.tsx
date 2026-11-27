import {ArrowCounterClockwise, Robot, User} from '@phosphor-icons/react';
import type {ActivityActor, ActivityEntry, ActivityKind} from '@sd/ts-client';
import {Button} from '@spacedrive/primitives';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';
import {useUndoDialog} from '../modals/UndoModal';

const REFRESH_MS = 10_000;

const VERBS: Record<ActivityKind, string> = {
	create: 'created',
	make_dir: 'made folder',
	modify: 'edited',
	move: 'moved',
	delete: 'deleted',
	attributes: 'changed attributes of'
};

/** Changes at or under `path`, newest first, from `activity.list`. */
export function useActivity(path: string, limit: number) {
	return useLibraryQuery(
		{type: 'activity.list', input: {path, limit}},
		{refetchInterval: REFRESH_MS}
	);
}

export function actorName(actor: ActivityActor): string {
	if (actor.kind === 'agent') return actor.label || 'An agent';
	return actor.device ? `You on ${actor.device}` : 'You';
}

/**
 * Who changed what under a folder, or to one file. Agent changes carry an
 * Undo that reverses the write from its journal.
 */
export function ActivityFeed({path}: {path: string}) {
	const activity = useActivity(path, 50);
	const openUndo = useUndoDialog();
	const entries = activity.data?.entries ?? [];

	if (activity.isLoading) {
		return (
			<p className="text-sidebar-inkDull px-2 pt-2 text-xs">
				Loading activity…
			</p>
		);
	}
	if (entries.length === 0) {
		return (
			<p className="text-sidebar-inkDull px-2 pt-2 text-xs">
				No changes made through Spacedrive here yet. Agent writes and
				file operations show up here with who made them.
			</p>
		);
	}

	return (
		<div className="space-y-0.5">
			{entries.map((entry) => (
				<div
					key={entry.id}
					className="hover:bg-app-box/40 flex items-start gap-2.5 rounded-lg p-2 transition-colors"
				>
					<span className="text-sidebar-inkDull mt-0.5 shrink-0">
						{entry.actor.kind === 'agent' ? (
							<Robot
								size={15}
								weight="bold"
								className="text-accent"
							/>
						) : (
							<User size={15} weight="bold" />
						)}
					</span>
					<div className="min-w-0 flex-1">
						<div className="text-sidebar-ink text-xs">
							<span className="font-medium">
								{actorName(entry.actor)}
							</span>{' '}
							{VERBS[entry.kind]}{' '}
							<span className="font-medium" title={entry.path}>
								{shown(entry.path, path)}
							</span>
							{entry.to_path && (
								<>
									{' '}
									to{' '}
									<span
										className="font-medium"
										title={entry.to_path}
									>
										{shown(entry.to_path, path)}
									</span>
								</>
							)}
						</div>
						<div className="text-sidebar-inkDull mt-0.5 text-[11px]">
							{ago(entry.at)}
							{entry.actor.kind === 'person' &&
								` · ${jobLabel(entry.actor.job)}`}
							{entry.changes > 1 && ` · ${entry.changes} changes`}
						</div>
					</div>
					{entry.actor.kind === 'agent' && entry.undoable && (
						<Button
							type="button"
							size="xs"
							variant="gray"
							className="shrink-0"
							onClick={() =>
								openUndo({
									job: entry.id,
									label: `${actorName(entry.actor)}'s change to ${baseName(entry.path)}`,
									onComplete: () => void activity.refetch()
								})
							}
						>
							<ArrowCounterClockwise
								size={12}
								className="mr-1 inline"
							/>
							Undo
						</Button>
					)}
				</div>
			))}
		</div>
	);
}

/** The latest change to one file, as a short "who, when" line. */
export function ChangedBy({entry}: {entry: ActivityEntry}) {
	return (
		<span
			title={`${VERBS[entry.kind]} ${new Date(entry.at).toLocaleString()}`}
		>
			{actorName(entry.actor)} · {ago(entry.at)}
		</span>
	);
}

function shown(path: string, scope: string): string {
	if (path === scope) return baseName(path);
	const prefix = scope.endsWith('/') ? scope : `${scope}/`;
	return path.startsWith(prefix) ? path.slice(prefix.length) : baseName(path);
}

function baseName(path: string): string {
	const parts = path.split(/[\\/]/).filter(Boolean);
	return parts[parts.length - 1] ?? path;
}

function jobLabel(job: string): string {
	const words = job.replace(/_/g, ' ');
	return words.charAt(0).toUpperCase() + words.slice(1);
}

function ago(iso: string): string {
	const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
	if (seconds < 60) return 'just now';
	if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
	if (seconds < 86_400) return `${Math.floor(seconds / 3600)} h ago`;
	return new Date(iso).toLocaleDateString(undefined, {
		month: 'short',
		day: 'numeric'
	});
}
