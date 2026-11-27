import {ArrowCounterClockwise, Desktop, Robot, User} from '@phosphor-icons/react';
import type {Actor, Device, ListLibraryDevicesInput, Space, SpaceChange} from '@sd/ts-client';
import {Button} from '@spacedrive/primitives';
import {useQuery} from '@tanstack/react-query';
import {useLibraryQuery, useNormalizedQuery} from '../../contexts/SpacedriveContext';
import {useUndoDialog} from '../modals/UndoModal';

const REFRESH_MS = 10_000;

const VERBS: Record<string, string> = {
	create: 'created',
	mkdir: 'made folder',
	modify: 'edited',
	move: 'moved',
	delete: 'deleted'
};

/** The namespace `actors::this_person` derives the person on a device from. */
const PERSON_NAMESPACE = '2b8e510c-9347-4d1a-a66f-0e953c71d248';

function uuidBytes(uuid: string): Uint8Array {
	const hex = uuid.replace(/-/g, '');
	return Uint8Array.from({length: 16}, (_, i) => parseInt(hex.slice(i * 2, i * 2 + 2), 16));
}

/** UUID v5 of `name` in `namespace`, as the core's `Uuid::new_v5` computes it. */
async function uuidV5(namespace: string, name: Uint8Array): Promise<string> {
	const input = new Uint8Array(16 + name.length);
	input.set(uuidBytes(namespace));
	input.set(name, 16);
	const bytes = new Uint8Array(await crypto.subtle.digest('SHA-1', input)).slice(0, 16);
	bytes[6] = (bytes[6]! & 0x0f) | 0x50;
	bytes[8] = (bytes[8]! & 0x3f) | 0x80;
	const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** The actor id of the person using this device, so their changes read as "You". */
function useLocalPersonId() {
	const {data: devices} = useNormalizedQuery<ListLibraryDevicesInput, Device[]>({
		query: 'devices.list',
		input: {include_offline: true, include_details: false},
		resourceType: 'device'
	});
	const deviceId = devices?.find((device) => device.is_current)?.id;
	return useQuery({
		queryKey: ['local-person-actor', deviceId],
		queryFn: () => uuidV5(PERSON_NAMESPACE, uuidBytes(deviceId!)),
		enabled: !!deviceId,
		staleTime: Infinity
	}).data;
}

function actorName(actor: Actor, you: string | undefined) {
	return actor.kind === 'person' && actor.id === you ? 'You' : actor.label;
}

/** The innermost space holding `path` on this device, and `path` relative to its root. */
function spaceOf(spaces: Space[] | undefined, path: string) {
	const target = path.replace(/\\/g, '/');
	let found: {space: Space; root: string; relative: string} | undefined;
	for (const space of spaces ?? []) {
		if (!space.path) continue;
		// A filesystem root trims to '' so every absolute path still matches its '/' prefix.
		const root = space.path.replace(/\\/g, '/').replace(/\/+$/, '');
		const inside = target === root || target.startsWith(`${root}/`);
		if (!inside || (found && found.root.length >= root.length)) continue;
		found = {space, root, relative: target.slice(root.length + 1)};
	}
	return found;
}

/**
 * The newest changes at or under `path`, from the event log of the space
 * holding it. A path in no space has no log, so `space` is undefined.
 */
export function useSpaceChanges(path: string, limit: number) {
	const spaces = useLibraryQuery(
		{type: 'spaces.list', input: {source_id: null}},
		{refetchInterval: REFRESH_MS}
	);
	const scope = spaceOf(spaces.data?.spaces, path);
	const changes = useLibraryQuery(
		{
			type: 'spaces.changes',
			input: {
				id: scope?.space.id ?? '',
				order: 'newest',
				path: scope?.relative || null,
				limit
			}
		},
		{enabled: !!scope, refetchInterval: REFRESH_MS}
	);
	return {
		space: scope?.space,
		relative: scope?.relative ?? '',
		changes: changes.data?.changes ?? [],
		isLoading: spaces.isLoading || (!!scope && changes.isLoading),
		refetch: changes.refetch
	};
}

function ActorIcon({actor}: {actor: Actor}) {
	if (actor.kind === 'agent')
		return <Robot size={15} weight="bold" className="text-accent" />;
	if (actor.kind === 'device') return <Desktop size={15} weight="bold" />;
	return <User size={15} weight="bold" />;
}

/** One line for a change: who did what, or the log's own summary for a change seen on disk. */
function describe(change: SpaceChange, scope: string, you: string | undefined) {
	if (change.origin === 'watcher') return change.summary;
	const verb = VERBS[change.op] ?? change.op;
	const target = change.to_path
		? `${shown(change.path, scope)} to ${shown(change.to_path, scope)}`
		: shown(change.path, scope);
	return `${actorName(change.actor, you)} ${verb} ${target}`;
}

/**
 * Who changed what under a folder, or to one file, from its space's event
 * log. Agent changes carry an Undo that reverses the write from its journal.
 */
export function ActivityFeed({path}: {path: string}) {
	const {space, relative, changes, isLoading, refetch} = useSpaceChanges(path, 50);
	const openUndo = useUndoDialog();
	const you = useLocalPersonId();

	if (isLoading) {
		return <p className="text-sidebar-inkDull px-2 pt-2 text-xs">Loading activity…</p>;
	}
	if (!space) {
		return (
			<p className="text-sidebar-inkDull px-2 pt-2 text-xs">
				Changes are recorded inside spaces. Make this folder, or one
				above it, a space to see who changes what here.
			</p>
		);
	}
	if (changes.length === 0) {
		return (
			<p className="text-sidebar-inkDull px-2 pt-2 text-xs">
				No changes in {space.name} here yet. Edits by you, your agents
				and other apps show up here with who made them.
			</p>
		);
	}

	return (
		<div className="space-y-0.5">
			{changes.map((change) => (
				<div
					key={change.seq}
					className="hover:bg-app-box/40 flex items-start gap-2.5 rounded-lg p-2 transition-colors"
				>
					<span className="text-sidebar-inkDull mt-0.5 shrink-0">
						<ActorIcon actor={change.actor} />
					</span>
					<div className="min-w-0 flex-1">
						<div className="text-sidebar-ink break-words text-xs" title={change.summary}>
							{describe(change, relative, you)}
						</div>
						<div className="text-sidebar-inkDull mt-0.5 text-[11px]">{ago(change.at)}</div>
					</div>
					{change.origin === 'agent_write' && change.undo_id && (
						<Button
							type="button"
							size="xs"
							variant="gray"
							className="shrink-0"
							onClick={() =>
								openUndo({
									job: change.undo_id!,
									label: `${change.actor.label}'s change to ${baseName(change.to_path ?? change.path)}`,
									onComplete: () => void refetch()
								})
							}
						>
							<ArrowCounterClockwise size={12} className="mr-1 inline" />
							Undo
						</Button>
					)}
				</div>
			))}
		</div>
	);
}

/** The latest change to one file, as a short "who, when" line. */
export function ChangedBy({change}: {change: SpaceChange}) {
	const you = useLocalPersonId();
	const who =
		change.origin === 'watcher'
			? `Outside Spacedrive on ${change.actor.label}`
			: actorName(change.actor, you);
	return (
		<span title={`${change.summary}, ${new Date(change.at).toLocaleString()}`}>
			{who} · {ago(change.at)}
		</span>
	);
}

function shown(path: string, scope: string): string {
	if (!scope) return path;
	if (path === scope) return baseName(path);
	return path.startsWith(`${scope}/`) ? path.slice(scope.length + 1) : path;
}

function baseName(path: string): string {
	const parts = path.split('/').filter(Boolean);
	return parts[parts.length - 1] ?? path;
}

function ago(iso: string): string {
	const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000);
	if (seconds < 60) return 'just now';
	if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
	if (seconds < 86_400) return `${Math.floor(seconds / 3600)} h ago`;
	return new Date(iso).toLocaleDateString(undefined, {month: 'short', day: 'numeric'});
}
