import {Check, Copy, Robot, Warning} from '@phosphor-icons/react';
import type {CreateGrantOutput, GrantCap, GrantInfo} from '@sd/ts-client';
import {
	Button,
	Dialog,
	dialogManager,
	Input,
	Select,
	SelectOption,
	toast,
	useDialog,
	type UseDialogProps
} from '@spacedrive/primitives';
import {useMemo, useState} from 'react';
import {useForm} from 'react-hook-form';
import {
	useCoreMutation,
	useCoreQuery,
	useLibraryMutation,
	useLibraryQuery
} from '../../contexts/SpacedriveContext';
import {agentSetupSnippets, type AgentSetupSnippet} from './agentSetup';

const MINUTE = 60;
const HOUR = 60 * MINUTE;

/** The lifetimes a task grant may have, 5 minutes to 12 hours. */
const LIFETIMES: {seconds: number; label: string}[] = [
	{seconds: 5 * MINUTE, label: '5 minutes'},
	{seconds: 15 * MINUTE, label: '15 minutes'},
	{seconds: 30 * MINUTE, label: '30 minutes'},
	{seconds: HOUR, label: '1 hour'},
	{seconds: 2 * HOUR, label: '2 hours'},
	{seconds: 4 * HOUR, label: '4 hours'},
	{seconds: 8 * HOUR, label: '8 hours'},
	{seconds: 12 * HOUR, label: '12 hours'}
];
const DEFAULT_LIFETIME = 2 * HOUR;

const ACCESS: {cap: GrantCap; label: string; hint: string}[] = [
	{cap: 'read', label: 'Read only', hint: 'List, read, grep and search.'},
	{
		cap: 'read_write',
		label: 'Read and write',
		hint: 'Also write, move and delete. Every change can be undone. AGENTS.md, CLAUDE.md, SKILL.md and .git stay protected.'
	}
];

interface GiveToAgentDialogProps {
	id: number;
	/** The folder on this device, as the daemon sees it. */
	path: string;
	name: string;
}

/**
 * Lend a folder to an agent: mint a grant with the chosen access and
 * lifetime, then show the setup to paste into the agent once, since the
 * token is never shown again. Active grants are listed beside it with
 * extend and revoke.
 */
export function useGiveToAgentDialog() {
	return (options: Omit<GiveToAgentDialogProps, 'id'>) =>
		dialogManager.create((props: UseDialogProps) => (
			<GiveToAgentDialog
				{...(props as GiveToAgentDialogProps)}
				{...options}
			/>
		));
}

function GiveToAgentDialog(props: GiveToAgentDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [cap, setCap] = useState<GrantCap>('read_write');
	const [lifetime, setLifetime] = useState(DEFAULT_LIFETIME);
	const [label, setLabel] = useState('');
	const [failure, setFailure] = useState<string | null>(null);
	const [minted, setMinted] = useState<CreateGrantOutput | null>(null);

	const setup = useCoreQuery({type: 'grants.setup', input: {}});
	const grants = useLibraryQuery({
		type: 'grants.list',
		input: {include_inactive: false}
	});
	const create = useLibraryMutation('grants.create', {
		onSuccess: () => grants.refetch()
	});
	const close = () => dialogManager.setState(props.id, {open: false});

	const submit = async () => {
		if (minted) return close();
		setFailure(null);
		try {
			setMinted(
				await create.mutateAsync({
					path: props.path,
					caps: cap,
					ttl_seconds: lifetime,
					label: label.trim() || null
				})
			);
		} catch (error) {
			setFailure(error instanceof Error ? error.message : String(error));
		}
	};

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`Give ${props.name} to an agent`}
			icon={<Robot size={20} weight="bold" />}
			ctaLabel={minted ? 'Done' : 'Create access'}
			submitDisabled={create.isPending}
			loading={create.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[600px] !max-w-[600px]"
		>
			<div className="space-y-4 py-1">
				{minted ? (
					<SetupPanel
						grant={minted.grant}
						token={minted.token}
						url={setup.data?.mcp_url}
						skill={setup.data?.skill}
					/>
				) : (
					<>
						<div className="grid grid-cols-2 gap-2">
							{ACCESS.map((option) => (
								<button
									key={option.cap}
									type="button"
									onClick={() => setCap(option.cap)}
									className={`rounded-md border px-3 py-2 text-left transition-colors ${
										cap === option.cap
											? 'border-accent bg-accent/10'
											: 'border-app-line bg-app-box hover:bg-app-hover'
									}`}
								>
									<div className="text-ink text-sm font-medium">
										{option.label}
									</div>
									<div className="text-ink-dull mt-0.5 text-[11px] leading-snug">
										{option.hint}
									</div>
								</button>
							))}
						</div>
						<div className="flex items-center gap-2">
							<Input
								size="sm"
								className="flex-1"
								value={label}
								maxLength={255}
								onChange={(event) =>
									setLabel(event.target.value)
								}
								placeholder="Agent name (optional)"
							/>
							<Select<string>
								value={String(lifetime)}
								size="sm"
								onChange={(value) => setLifetime(Number(value))}
							>
								{LIFETIMES.map((option) => (
									<SelectOption
										key={option.seconds}
										value={String(option.seconds)}
									>
										{option.label}
									</SelectOption>
								))}
							</Select>
						</div>
						<div className="text-ink-dull text-[11px]">
							The agent can reach {props.path} and nothing outside
							it. Access ends on its own after the lifetime, or
							when you revoke it.
						</div>
					</>
				)}
				{setup.data && !setup.data.endpoint_enabled && <EndpointOff />}
				{failure && (
					<div className="bg-status-error/10 text-status-error rounded-md px-3 py-2 text-xs">
						{failure}
					</div>
				)}
				<ActiveGrants
					grants={grants.data?.grants ?? []}
					folder={props.path}
					onChange={() => grants.refetch()}
				/>
			</div>
		</Dialog>
	);
}

function SetupPanel(props: {
	grant: GrantInfo;
	token: string;
	url?: string;
	skill?: string;
}) {
	const snippets = useMemo(
		() => (props.url ? agentSetupSnippets(props.url, props.token) : []),
		[props.url, props.token]
	);
	const tabs: AgentSetupSnippet[] = [
		...snippets,
		...(props.skill
			? [
					{
						id: 'skill',
						label: 'Skill',
						hint: "What the agent's get_skill tool returns. Paste it into a skill file or a prompt.",
						text: props.skill
					}
				]
			: [])
	];
	const [tab, setTab] = useState('claude');
	const current = tabs.find((snippet) => snippet.id === tab) ?? tabs[0];

	return (
		<div className="space-y-3">
			<div className="bg-app-box text-ink-dull flex items-start gap-2 rounded-md px-3 py-2 text-xs">
				<Warning
					size={14}
					className="text-status-warning mt-0.5 shrink-0"
				/>
				<span>
					{props.grant.caps === 'read_write'
						? 'Read and write'
						: 'Read only'}{' '}
					access until {formatTime(props.grant.expires_at)}. Copy the
					setup now: the token is shown once.
				</span>
			</div>
			<div className="border-app-line flex gap-1 border-b">
				{tabs.map((snippet) => (
					<button
						key={snippet.id}
						type="button"
						onClick={() => setTab(snippet.id)}
						className={`-mb-px border-b-2 px-2.5 py-1.5 text-xs transition-colors ${
							current?.id === snippet.id
								? 'border-accent text-ink'
								: 'text-ink-dull hover:text-ink border-transparent'
						}`}
					>
						{snippet.label}
					</button>
				))}
			</div>
			{current && (
				<div className="space-y-1.5">
					<div className="text-ink-dull text-[11px]">
						{current.hint}
					</div>
					<CodeBlock text={current.text} />
					{current.shell && (
						<>
							<div className="text-ink-dull pt-1 text-[11px]">
								Then, in the shell that starts the agent. Keep
								this line out of files you commit.
							</div>
							<CodeBlock text={current.shell} />
						</>
					)}
				</div>
			)}
		</div>
	);
}

function CodeBlock({text}: {text: string}) {
	const [copied, setCopied] = useState(false);
	const copy = async () => {
		try {
			await navigator.clipboard.writeText(text);
			setCopied(true);
			setTimeout(() => setCopied(false), 1500);
		} catch (error) {
			toast.error(`Could not copy: ${error}`);
		}
	};
	return (
		<div className="relative">
			<pre className="bg-app-darkBox text-ink max-h-56 overflow-auto whitespace-pre-wrap break-all rounded-md px-3 py-2 pr-10 font-mono text-[11px] leading-relaxed">
				{text}
			</pre>
			<button
				type="button"
				onClick={copy}
				title="Copy"
				className="text-ink-dull hover:bg-app-hover hover:text-ink absolute right-2 top-2 rounded p-1"
			>
				{copied ? <Check size={14} /> : <Copy size={14} />}
			</button>
		</div>
	);
}

function EndpointOff() {
	const [requested, setRequested] = useState(false);
	const update = useCoreMutation('config.app.update');
	const turnOn = async () => {
		try {
			await update.mutateAsync({agent_mcp_enabled: true});
			setRequested(true);
		} catch (error) {
			toast.error(`Could not turn on the agent endpoint: ${error}`);
		}
	};
	return (
		<div className="bg-app-box flex items-center justify-between gap-3 rounded-md px-3 py-2">
			<div className="text-ink-dull text-xs">
				{requested
					? 'The agent endpoint starts the next time Spacedrive starts. It listens on this computer only.'
					: 'The agent endpoint is off, so an agent cannot connect yet. It listens on this computer only.'}
			</div>
			{!requested && (
				<Button
					type="button"
					size="sm"
					className="shrink-0 whitespace-nowrap"
					variant="accent"
					onClick={turnOn}
					disabled={update.isPending}
				>
					Turn on
				</Button>
			)}
		</div>
	);
}

const EXTENSIONS = [
	{seconds: HOUR, label: '+1 h'},
	{seconds: 4 * HOUR, label: '+4 h'}
];
const MAX_LIFETIME = 12 * HOUR;

/** Seconds a grant has left. `grants.extend` sets the end from now, so an
 * extension asks for what is left plus the added time. */
function remaining(grant: GrantInfo): number {
	return Math.max(
		0,
		Math.floor((new Date(grant.expires_at).getTime() - Date.now()) / 1000)
	);
}

function ActiveGrants(props: {
	grants: GrantInfo[];
	folder: string;
	onChange: () => void;
}) {
	const extend = useLibraryMutation('grants.extend', {
		onSuccess: props.onChange
	});
	const revoke = useLibraryMutation('grants.revoke', {
		onSuccess: props.onChange
	});
	if (props.grants.length === 0) return null;
	const here = (grant: GrantInfo) => grant.subtree === props.folder;
	const ordered = [...props.grants].sort(
		(a, b) => Number(here(b)) - Number(here(a))
	);

	const run = async (action: Promise<unknown>, done: string) => {
		try {
			await action;
			toast.success(done);
		} catch (error) {
			toast.error(error instanceof Error ? error.message : String(error));
		}
	};

	return (
		<div className="space-y-1.5">
			<div className="text-ink-faint text-[11px] font-medium uppercase tracking-wide">
				Active access
			</div>
			<div className="divide-app-line border-app-line divide-y overflow-hidden rounded-md border">
				{ordered.map((grant) => (
					<div
						key={grant.id}
						className="flex items-center gap-3 px-3 py-2"
					>
						<div className="min-w-0 flex-1">
							<div className="text-ink truncate text-sm">
								{grant.label || 'Unnamed agent'}
								<span className="text-ink-dull ml-2 text-[11px]">
									{grant.caps === 'read_write'
										? 'read and write'
										: 'read only'}
								</span>
							</div>
							<div
								className="text-ink-dull truncate text-[11px]"
								title={grant.subtree}
							>
								{here(grant) ? 'This folder' : grant.subtree} ·
								ends {formatTime(grant.expires_at)}
								{grant.last_used_at
									? ` · last used ${formatTime(grant.last_used_at)}`
									: ' · not used yet'}
							</div>
						</div>
						{EXTENSIONS.map((option) => (
							<Button
								type="button"
								key={option.seconds}
								size="sm"
								variant="gray"
								title={`Add ${option.label.slice(1)}, up to 12 h from now`}
								disabled={
									extend.isPending ||
									remaining(grant) >= MAX_LIFETIME
								}
								onClick={() =>
									run(
										extend.mutateAsync({
											id: grant.id,
											ttl_seconds: Math.min(
												MAX_LIFETIME,
												remaining(grant) +
													option.seconds
											)
										}),
										'Access extended'
									)
								}
							>
								{option.label}
							</Button>
						))}
						<Button
							type="button"
							size="sm"
							variant="gray"
							disabled={revoke.isPending}
							onClick={() =>
								run(
									revoke.mutateAsync({id: grant.id}),
									'Access revoked'
								)
							}
						>
							Revoke
						</Button>
					</div>
				))}
			</div>
		</div>
	);
}

function formatTime(iso: string): string {
	const at = new Date(iso);
	const sameDay = at.toDateString() === new Date().toDateString();
	return at.toLocaleString(undefined, {
		hour: 'numeric',
		minute: '2-digit',
		...(sameDay ? {} : {month: 'short', day: 'numeric'})
	});
}
