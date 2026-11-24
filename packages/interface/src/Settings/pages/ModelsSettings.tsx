import { CheckCircle, CircleNotch } from "@phosphor-icons/react";
import {
	useJobs,
	type Category,
	type CloudCaptionStatus,
	type ExtendedJobListItem,
	type JsonValue,
	type ListProvidersOutput,
} from "@sd/ts-client";
import {
	Button,
	Input,
	PasswordInput,
	ProgressBar,
	Switch,
} from "@spacedrive/primitives";
import { useState, type ReactNode } from "react";
import { usePlatform } from "../../contexts/PlatformContext";
import {
	useCoreMutation,
	useCoreQuery,
	useLibraryMutation,
	useLibraryQuery,
	useSpacedriveClient,
} from "../../contexts/SpacedriveContext";
import { formatBytes } from "../../routes/overview/DevicePanel";
import {
	Section,
	SectionCard,
	SectionRows,
	SettingItem,
	SettingsEmptyState,
	SettingsNote,
	SettingsPageContent,
	SettingsSkeleton,
} from "../Layout";

const CATEGORIES: Record<Category, { label: string; description: string }> = {
	face_detection: {
		label: "Face detection",
		description: "Finds faces in photos and video frames.",
	},
	face_embedding: {
		label: "Face recognition",
		description: "Groups faces of the same person.",
	},
	scene_classification: {
		label: "Scene labels",
		description: "Tags photos with places and settings like beach or kitchen.",
	},
	image_caption: {
		label: "Image captions",
		description: "Writes a short description of a photo or shot.",
	},
	text_embedding: {
		label: "Text search",
		description: "Matches searches to transcripts and captions by meaning.",
	},
};

const CATEGORY_ORDER = Object.keys(CATEGORIES) as Category[];

const DOWNLOAD_ACTIONS = ["ai.models.download", "models.whisper.download"];

/** A row the page renders, from either the inference catalog or Whisper. */
interface ModelEntry {
	id: string;
	name: string;
	description: string | null;
	license: string | null;
	licenseUrl: string | null;
	sizeBytes: number;
	installed: boolean;
	active: boolean;
	download: () => void;
	remove: () => void;
}

interface Download {
	jobIds: string[];
	progress: number;
}

function field(value: JsonValue | null, key: string): JsonValue {
	if (value === null || typeof value !== "object" || Array.isArray(value))
		return null;
	return value[key] ?? null;
}

/**
 * Running download jobs by model id. The core tags each download job with
 * the model it fetches, so a download started elsewhere or before the
 * window opened still shows here. A model of several files has one job per
 * file, weighted by size.
 */
function downloadsByModel(jobs: ExtendedJobListItem[]) {
	const totals = new Map<
		string,
		{ jobIds: string[]; done: number; total: number }
	>();
	for (const job of jobs) {
		const action = job.action_context;
		if (!action || !DOWNLOAD_ACTIONS.includes(action.action_type)) continue;
		if (
			job.status !== "queued" &&
			job.status !== "running" &&
			job.status !== "paused"
		)
			continue;
		const modelId = field(action.action_input, "model_id");
		if (typeof modelId !== "string") continue;
		const size = field(action.context, "size_bytes");
		const weight = typeof size === "number" && size > 0 ? size : 1;
		const entry = totals.get(modelId) ?? { jobIds: [], done: 0, total: 0 };
		entry.jobIds.push(job.id);
		entry.done += job.progress * weight;
		entry.total += weight;
		totals.set(modelId, entry);
	}
	const downloads = new Map<string, Download>();
	for (const [modelId, entry] of totals) {
		downloads.set(modelId, {
			jobIds: entry.jobIds,
			progress: entry.total ? entry.done / entry.total : 0,
		});
	}
	return downloads;
}

function Chip({ children }: { children: ReactNode }) {
	return (
		<span className="bg-app-input text-tiny text-ink-dull rounded-full px-2 py-1">
			{children}
		</span>
	);
}

function ModelRow({
	model,
	download,
	busy,
	onCancel,
}: {
	model: ModelEntry;
	download: Download | undefined;
	busy: boolean;
	onCancel: (jobIds: string[]) => void;
}) {
	const percent = download ? Math.round(download.progress * 100) : 0;
	return (
		<div className="space-y-2.5 px-4 py-3.5">
			<div className="flex items-start justify-between gap-4">
				<div className="min-w-0">
					<div className="flex items-center gap-1.5">
						{model.installed && (
							<CheckCircle
								className="text-status-success size-4 shrink-0"
								weight="fill"
							/>
						)}
						<p className="text-ink text-sm font-medium">{model.name}</p>
						{model.active && (
							<span className="bg-accent/15 text-tiny text-accent rounded-full px-2 py-0.5">
								Active
							</span>
						)}
					</div>
					{model.description && (
						<p className="text-tiny text-ink-faint mt-0.5 leading-snug">
							{model.description}
						</p>
					)}
					<div className="mt-1.5 flex flex-wrap gap-1.5">
						<Chip>{formatBytes(model.sizeBytes)}</Chip>
						{model.license &&
							(model.licenseUrl ? (
								<a
									href={model.licenseUrl}
									target="_blank"
									rel="noreferrer"
									className="bg-app-input text-tiny text-ink-dull hover:text-ink rounded-full px-2 py-1"
								>
									{model.license}
								</a>
							) : (
								<Chip>{model.license}</Chip>
							))}
						<Chip>{model.installed ? "Installed" : "Not installed"}</Chip>
					</div>
				</div>

				<div className="flex shrink-0 gap-2">
					{download ? (
						<Button
							variant="gray"
							size="sm"
							onClick={() => onCancel(download.jobIds)}
						>
							Cancel
						</Button>
					) : model.installed ? (
						<Button
							variant="gray"
							size="sm"
							disabled={busy}
							onClick={model.remove}
						>
							Delete
						</Button>
					) : (
						<Button
							variant="accent"
							size="sm"
							disabled={busy}
							onClick={model.download}
						>
							{busy ? (
								<CircleNotch className="size-3.5 animate-spin" />
							) : (
								"Download"
							)}
						</Button>
					)}
				</div>
			</div>
			{download && (
				<div className="flex items-center gap-3">
					<div className="flex-1">
						<ProgressBar percent={percent} />
					</div>
					<span className="text-tiny text-ink-faint w-10 text-right tabular-nums">
						{percent}%
					</span>
				</div>
			)}
		</div>
	);
}

function ActiveProviders({
	providers,
	whisperActive,
}: {
	providers: ListProvidersOutput;
	whisperActive: string | null;
}) {
	const describe = (category: Category) => {
		const active = providers.active.find((a) => a.category === category);
		if (!active) return "None";
		if (active.locality === "cloud")
			return `Cloud, ${providers.cloud_caption.model ?? "unnamed model"}`;
		return (
			providers.models.find((model) => model.id === active.model_id)?.name ??
			"On this device"
		);
	};

	return (
		<SectionRows>
			<SettingItem label="Speech to text">
				<span className="text-ink-dull text-xs">{whisperActive ?? "None"}</span>
			</SettingItem>
			{CATEGORY_ORDER.map((category) => (
				<SettingItem key={category} label={CATEGORIES[category].label}>
					<span className="text-ink-dull text-xs">{describe(category)}</span>
				</SettingItem>
			))}
		</SectionRows>
	);
}

function CloudCaptionForm({
	status,
	onSaved,
}: {
	status: CloudCaptionStatus;
	onSaved: () => void;
}) {
	const [baseUrl, setBaseUrl] = useState(status.base_url ?? "");
	const [model, setModel] = useState(status.model ?? "");
	const [apiKey, setApiKey] = useState("");
	const configure = useLibraryMutation("ai.cloud_caption.configure", {
		onSuccess: () => {
			setApiKey("");
			onSaved();
		},
	});

	const unchanged = {
		enabled: null,
		base_url: null,
		model: null,
		api_key: null,
		clear_key: false,
	};
	const dirty =
		baseUrl.trim() !== (status.base_url ?? "") ||
		model.trim() !== (status.model ?? "") ||
		apiKey.trim() !== "";
	const configured = Boolean(status.base_url && status.model && status.has_key);

	return (
		<SectionCard className="divide-app-line divide-y">
			<SettingItem
				label="Use the cloud for captions"
				description={
					configured
						? "Used only when no local caption model is installed, and only for extensions that allow it."
						: "Save an endpoint, a model and a key first."
				}
			>
				<Switch
					size="sm"
					checked={status.enabled}
					disabled={!configured || configure.isPending}
					onCheckedChange={(enabled) =>
						configure.mutate({ ...unchanged, enabled })
					}
				/>
			</SettingItem>
			<div className="space-y-3 px-4 py-4">
				<label className="block">
					<span className="text-ink-dull mb-1.5 block text-xs">Endpoint</span>
					<Input
						className="w-full"
						inputElementClassName="font-mono"
						placeholder="https://api.example.com/v1"
						value={baseUrl}
						onChange={(e) => setBaseUrl(e.target.value)}
					/>
				</label>
				<label className="block">
					<span className="text-ink-dull mb-1.5 block text-xs">Model</span>
					<Input
						className="w-full"
						inputElementClassName="font-mono"
						placeholder="Model name"
						value={model}
						onChange={(e) => setModel(e.target.value)}
					/>
				</label>
				<label className="block">
					<span className="text-ink-dull mb-1.5 block text-xs">API key</span>
					<PasswordInput
						className="w-full"
						inputElementClassName="font-mono"
						autoComplete="off"
						placeholder={
							status.has_key ? "Stored. Enter a new key to replace it." : "Key"
						}
						value={apiKey}
						onChange={(e) => setApiKey(e.target.value)}
					/>
				</label>
				{configure.error && (
					<p className="text-tiny text-status-error leading-relaxed">
						{configure.error.message}
					</p>
				)}
				<div className="flex justify-end gap-2">
					{status.has_key && (
						<Button
							variant="gray"
							size="sm"
							disabled={configure.isPending}
							onClick={() =>
								configure.mutate({
									...unchanged,
									clear_key: true,
								})
							}
						>
							Remove key
						</Button>
					)}
					<Button
						variant="accent"
						size="sm"
						disabled={!dirty || configure.isPending}
						onClick={() =>
							configure.mutate({
								...unchanged,
								base_url: baseUrl.trim() || null,
								model: model.trim() || null,
								api_key: apiKey.trim() || null,
							})
						}
					>
						Save
					</Button>
				</div>
			</div>
		</SectionCard>
	);
}

export function ModelsSettings() {
	const platform = usePlatform();
	const client = useSpacedriveClient();
	const providersQuery = useLibraryQuery({
		type: "ai.providers.list",
		input: {},
	});
	const whisperQuery = useCoreQuery({ type: "models.whisper.list", input: {} });
	const refetch = () => {
		providersQuery.refetch();
		whisperQuery.refetch();
	};
	const { jobs, cancel } = useJobs({
		onJobCompleted: refetch,
		onJobFailed: refetch,
		onJobCancelled: refetch,
	});

	const downloadModel = useLibraryMutation("ai.models.download");
	const deleteModel = useLibraryMutation("ai.models.delete", {
		onSuccess: refetch,
	});
	const downloadWhisper = useLibraryMutation("models.whisper.download");
	const deleteWhisper = useCoreMutation("models.whisper.delete", {
		onSuccess: refetch,
	});

	const providers = providersQuery.data;
	const whisper = whisperQuery.data;
	const loadError = providersQuery.error ?? whisperQuery.error;
	if (loadError) {
		return (
			<SettingsPageContent>
				<SettingsNote>
					Spacedrive could not list models: {loadError.message}
				</SettingsNote>
			</SettingsPageContent>
		);
	}
	if (!providers || !whisper) return <SettingsSkeleton />;

	const downloads = downloadsByModel(jobs);
	const confirmDownload = (
		name: string,
		size: number,
		license: string | null,
		start: () => void,
	) => {
		const terms = license
			? ` It is licensed ${license}, and downloading it accepts that license.`
			: "";
		platform.confirm(
			`Download ${name} (${formatBytes(size)})?${terms}`,
			(confirmed) => confirmed && start(),
		);
	};
	const confirmDelete = (name: string, start: () => void) =>
		platform.confirm(
			`Delete ${name}? You can download it again later.`,
			(confirmed) => confirmed && start(),
		);
	const busy = (id: string) =>
		(downloadModel.isPending && downloadModel.variables?.model_id === id) ||
		(deleteModel.isPending && deleteModel.variables?.model_id === id) ||
		(downloadWhisper.isPending &&
			`whisper-${downloadWhisper.variables?.model}` === id) ||
		(deleteWhisper.isPending &&
			`whisper-${deleteWhisper.variables?.model}` === id);
	const actionError =
		downloadModel.error ??
		deleteModel.error ??
		downloadWhisper.error ??
		deleteWhisper.error;

	const whisperModels: ModelEntry[] = whisper.models.map((model) => {
		const size = model.id.replace(/^whisper-/, "");
		return {
			id: model.id,
			name: model.name,
			description: model.description,
			license: model.license,
			licenseUrl: null,
			sizeBytes: model.size_bytes,
			installed: model.downloaded,
			active: whisper.active === model.id,
			download: () =>
				confirmDownload(model.name, model.size_bytes, model.license, () =>
					downloadWhisper.mutate({ model: size }),
				),
			remove: () =>
				confirmDelete(model.name, () => deleteWhisper.mutate({ model: size })),
		};
	});

	const categoryModels = (category: Category): ModelEntry[] =>
		providers.models
			.filter((model) => model.category === category)
			.map((model) => ({
				id: model.id,
				name: model.name,
				description: model.description,
				license: model.license,
				licenseUrl: model.license_url,
				sizeBytes: model.size_bytes,
				installed: model.installed,
				active: providers.active.some((a) => a.model_id === model.id),
				download: () =>
					confirmDownload(model.name, model.size_bytes, model.license, () =>
						downloadModel.mutate({
							model_id: model.id,
							accept_license: model.license,
						}),
					),
				remove: () =>
					confirmDelete(model.name, () =>
						deleteModel.mutate({ model_id: model.id }),
					),
			}));

	const renderRows = (models: ModelEntry[]) => (
		<SectionCard className="divide-app-line divide-y">
			{models.map((model) => (
				<ModelRow
					key={model.id}
					model={model}
					download={downloads.get(model.id)}
					busy={busy(model.id)}
					onCancel={(jobIds) => jobIds.forEach((id) => cancel(id))}
				/>
			))}
		</SectionCard>
	);

	const categories = CATEGORY_ORDER.filter((category) =>
		providers.models.some((model) => model.category === category),
	);

	return (
		<SettingsPageContent>
			<Section
				title="Active Providers"
				description="What answers each kind of analysis. Analysis runs only when you start it."
			>
				<ActiveProviders
					providers={providers}
					whisperActive={
						whisper.models.find((model) => model.id === whisper.active)?.name ??
						null
					}
				/>
			</Section>

			{actionError && <SettingsNote>{actionError.message}</SettingsNote>}

			<Section
				title="Speech to Text"
				description="Transcribes the speech in videos and audio on this device."
			>
				{renderRows(whisperModels)}
			</Section>

			{!providers.inference_available ? (
				<SettingsNote>
					This build of Spacedrive cannot run on-device analysis models.
				</SettingsNote>
			) : categories.length === 0 ? (
				<SettingsEmptyState
					title="No analysis models available"
					description="This build has no models for faces, scenes, captions or text search yet."
				/>
			) : (
				categories.map((category) => (
					<Section
						key={category}
						title={CATEGORIES[category].label}
						description={CATEGORIES[category].description}
					>
						{renderRows(categoryModels(category))}
					</Section>
				))
			)}

			<Section
				title="Cloud Captions"
				description="Caption images through any service that speaks the OpenAI chat completions format, with your own key. Images leave this device only while this is on. The key is stored encrypted in the library and is never shown again."
			>
				<CloudCaptionForm
					key={`${client.getCurrentLibraryId()}|${providers.cloud_caption.base_url}|${providers.cloud_caption.model}`}
					status={providers.cloud_caption}
					onSaved={refetch}
				/>
			</Section>
		</SettingsPageContent>
	);
}
