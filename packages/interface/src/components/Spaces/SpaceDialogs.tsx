import {Planet, Robot, Trash} from '@phosphor-icons/react';
import type {Space, SpaceKind} from '@sd/ts-client';
import {
	Button,
	Dialog,
	dialogManager,
	Input,
	Label,
	Switch,
	toast,
	useDialog,
	type UseDialogProps
} from '@spacedrive/primitives';
import {useState} from 'react';
import {useForm} from 'react-hook-form';
import {useLibraryMutation} from '../../contexts/SpacedriveContext';
import {useGiveToAgentDialog} from '../modals/GiveToAgentModal';
import {useSpaces} from './useSpaces';

/** The paths every writable grant protects, from grants/scope.rs. */
const PROTECTED = ['AGENTS.md', 'CLAUDE.md', 'SKILL.md', '.git'];

function message(error: unknown) {
	return error instanceof Error ? error.message : String(error);
}

function ContentIndexing({
	checked,
	onChange
}: {
	checked: boolean;
	onChange: (checked: boolean) => void;
}) {
	return (
		<label className="bg-app-box flex cursor-pointer items-center justify-between gap-3 rounded-md px-3 py-2">
			<div>
				<div className="text-ink text-sm">Content indexing</div>
				<div className="text-ink-dull text-[11px]">
					Hash and read the files inside, so their text is searchable
					and duplicates are found. Names and sizes are indexed either
					way.
				</div>
			</div>
			<Switch checked={checked} onCheckedChange={onChange} size="sm" />
		</label>
	);
}

interface CreateSpaceProps {
	id: number;
	kind: SpaceKind;
	/** The folder, for a folder space. */
	path?: string;
	name?: string;
	onCreated?: (space: Space) => void;
}

/**
 * Make a folder a space, or create an empty workspace space. A folder space
 * changes nothing on disk; a workspace gets a new folder in the data
 * directory.
 */
export function useCreateSpaceDialog() {
	return (options: Omit<CreateSpaceProps, 'id'>) =>
		dialogManager.create((props: UseDialogProps) => (
			<CreateSpaceDialog {...(props as CreateSpaceProps)} {...options} />
		));
}

function CreateSpaceDialog(props: CreateSpaceProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const spaces = useSpaces();
	const [name, setName] = useState(props.name ?? '');
	const [contentIndexing, setContentIndexing] = useState(true);
	const [failure, setFailure] = useState<string | null>(null);
	const create = useLibraryMutation('spaces.create', {
		onSuccess: () => spaces.refetch()
	});
	const workspace = props.kind === 'workspace';

	const submit = async () => {
		setFailure(null);
		try {
			const {space} = await create.mutateAsync({
				kind: props.kind,
				name: name.trim() || null,
				path: props.path ?? null,
				content_indexing: contentIndexing
			});
			dialogManager.setState(props.id, {open: false});
			toast.success(`${space.name} is a space`);
			props.onCreated?.(space);
		} catch (error) {
			setFailure(message(error));
		}
	};

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={workspace ? 'New workspace' : `Make ${props.name ?? 'this folder'} a space`}
			icon={<Planet size={20} weight="bold" />}
			ctaLabel="Create space"
			submitDisabled={create.isPending || (workspace && !name.trim())}
			loading={create.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={() => dialogManager.setState(props.id, {open: false})}
		>
			<div className="space-y-4 py-1">
				<div className="text-ink-dull text-xs">
					{workspace
						? 'An empty folder for people and agents to work in. Spacedrive creates it in its data folder and tracks it on its own.'
						: 'A space is a folder you and your agents work in, with its own settings. Nothing on disk changes.'}
				</div>
				<div>
					<Label>Name</Label>
					<Input
						value={name}
						onChange={(event) => setName(event.target.value)}
						placeholder={workspace ? 'e.g. Research agent' : props.name}
						autoFocus
					/>
				</div>
				{props.path && (
					<div className="text-ink-dull truncate text-[11px]" title={props.path}>
						{props.path}
					</div>
				)}
				<ContentIndexing
					checked={contentIndexing}
					onChange={setContentIndexing}
				/>
				{failure && <div className="text-status-error text-xs">{failure}</div>}
			</div>
		</Dialog>
	);
}

interface SpaceSettingsProps {
	id: number;
	space: Space;
}

/** A space's settings: name, content indexing, and what comes later. */
export function useSpaceSettingsDialog() {
	return (space: Space) =>
		dialogManager.create((props: UseDialogProps) => (
			<SpaceSettingsDialog {...(props as SpaceSettingsProps)} space={space} />
		));
}

function SpaceSettingsDialog(props: SpaceSettingsProps) {
	const {space} = props;
	const dialog = useDialog(props);
	const form = useForm();
	const spaces = useSpaces();
	const openGiveToAgent = useGiveToAgentDialog();
	const [name, setName] = useState(space.name);
	const [contentIndexing, setContentIndexing] = useState(space.content_indexing);
	const [confirmDelete, setConfirmDelete] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);
	const update = useLibraryMutation('spaces.update', {
		onSuccess: () => spaces.refetch()
	});
	const remove = useLibraryMutation('spaces.delete', {
		onSuccess: () => spaces.refetch()
	});
	const close = () => dialogManager.setState(props.id, {open: false});
	const changed =
		name.trim() !== space.name || contentIndexing !== space.content_indexing;

	const submit = async () => {
		if (!changed) return close();
		setFailure(null);
		try {
			await update.mutateAsync({
				id: space.id,
				name: name.trim() !== space.name ? name.trim() : null,
				content_indexing:
					contentIndexing !== space.content_indexing ? contentIndexing : null
			});
			close();
		} catch (error) {
			setFailure(message(error));
		}
	};

	const deleteSpace = async () => {
		setFailure(null);
		try {
			await remove.mutateAsync({id: space.id});
			close();
			toast.success(`${space.name} is no longer a space. Its files are untouched.`);
		} catch (error) {
			setFailure(message(error));
		}
	};

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`${space.name} settings`}
			icon={<Planet size={20} weight="bold" />}
			ctaLabel={changed ? 'Save' : 'Done'}
			submitDisabled={update.isPending || !name.trim()}
			loading={update.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[480px] !max-w-[480px]"
		>
			<div className="space-y-4 py-1">
				<div>
					<Label>Name</Label>
					<Input value={name} onChange={(event) => setName(event.target.value)} />
					<div className="text-ink-dull mt-1 truncate text-[11px]" title={space.path ?? space.root}>
						{space.kind === 'workspace' ? 'Workspace' : 'Folder'} ·{' '}
						{space.path ?? 'Not on this device'}
					</div>
				</div>

				<ContentIndexing checked={contentIndexing} onChange={setContentIndexing} />

				<div className="bg-app-box flex items-center justify-between gap-3 rounded-md px-3 py-2 opacity-70">
					<div>
						<div className="text-ink text-sm">Versioning</div>
						<div className="text-ink-dull text-[11px]">
							History, snapshots and restore for this space. Not available yet.
						</div>
					</div>
					<span className="text-ink-dull text-xs">Off</span>
				</div>

				<div className="bg-app-box rounded-md px-3 py-2">
					<div className="text-ink text-sm">Protected from agents</div>
					<div className="text-ink-dull text-[11px]">
						Agents with write access cannot change these anywhere in the
						space. Per-space patterns come later.
					</div>
					<div className="mt-2 flex flex-wrap gap-1">
						{PROTECTED.map((pattern) => (
							<code
								key={pattern}
								className="bg-app-darkBox text-ink-dull rounded px-1.5 py-0.5 text-[11px]"
							>
								{pattern}
							</code>
						))}
					</div>
				</div>

				<div className="flex items-center justify-between gap-2">
					<Button
						type="button"
						variant="gray"
						size="sm"
						disabled={!space.path}
						onClick={() => {
							if (!space.path) return;
							close();
							openGiveToAgent({path: space.path, name: space.name});
						}}
					>
						<Robot size={14} weight="bold" className="mr-1.5 inline" />
						Give to an agent…
					</Button>
					{confirmDelete ? (
						<div className="flex items-center gap-2">
							<span className="text-ink-dull text-[11px]">Files stay on disk.</span>
							<Button
								type="button"
								variant="colored"
								className="bg-red-500 text-white"
								size="sm"
								disabled={remove.isPending}
								onClick={() => void deleteSpace()}
							>
								Remove space
							</Button>
						</div>
					) : (
						<Button
							type="button"
							variant="gray"
							size="sm"
							onClick={() => setConfirmDelete(true)}
						>
							<Trash size={14} weight="bold" className="mr-1.5 inline" />
							Remove space…
						</Button>
					)}
				</div>
				{failure && <div className="text-status-error text-xs">{failure}</div>}
			</div>
		</Dialog>
	);
}
