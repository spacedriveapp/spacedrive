import { useState, type ReactNode } from "react";
import { useForm } from "react-hook-form";
import { useNavigate, type NavigateFunction } from "react-router-dom";
import {
	ArrowCounterClockwise,
	Books,
	CheckCircle,
	HardDrive,
	Warning,
} from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	Input,
	RadixCheckbox,
	Switch,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type {
	SdPath,
	StorePlacement,
	TrackSourceOutput,
	ValidateSourcePathOutput,
	Volume,
} from "@sd/ts-client";
import clsx from "clsx";
import {
	useLibraryMutation,
	useLibraryQuery,
} from "../../../contexts/SpacedriveContext";
import { describeError, Failure } from "../../../components/modals/RearrangeModal";
import {
	addRefusal,
	buildPayload,
	changedKeys,
	containingVolume,
	effectiveDefaults,
	inclusionOf,
	libraryDefaults,
	overridesFrom,
	placementChoices,
	suggestedName,
	targetPath,
	type AddSettings,
	type AddTarget,
} from "./addToLibrary";
import {
	CategoryGrid,
	DrivePicker,
	FolderPicker,
	NetworkComingSoon,
	type StorageCategory,
} from "./AddToLibraryPicker";
import {
	CloudConnectionFields,
	CloudProviderGrid,
	cloudStepIcon,
	useCloudConnection,
	type CloudProvider,
} from "./CloudStorageSteps";

import FolderIcon from "@sd/assets/icons/Folder.webp";
import HDDIcon from "@sd/assets/icons/HDD.webp";
import ServerIcon from "@sd/assets/icons/Server.webp";

/**
 * Add to Library: one setup modal for a folder or a whole drive. It shows the
 * library's defaults for the chosen scope, lets the user change each one for
 * this add, and reports what the core saved. Opened with a target from the
 * Explorer, the path bar or a volume's menu; opened without one from Home,
 * it first asks what to add.
 */

interface AddToLibraryOptions {
	target?: AddTarget;
	/** Called with the added root, for a caller that wants to show it. */
	onAdded?: (root: SdPath) => void;
}

type Step =
	| { kind: "pick" }
	| { kind: "folder" }
	| { kind: "drive" }
	| { kind: "network" }
	| { kind: "cloud-provider" }
	| { kind: "cloud"; provider: CloudProvider }
	| { kind: "setup"; target: AddTarget; from?: Step }
	| { kind: "done"; target: AddTarget; result: TrackSourceOutput | null };

/**
 * Dialogs mount outside the router, so the hook captures navigation where it
 * is called and hands it to the dialog for its jobs link.
 */
export function useAddToLibraryDialog() {
	const navigate = useNavigate();
	return (options: AddToLibraryOptions = {}) =>
		dialogManager.create((props: UseDialogProps) => (
			<AddToLibraryDialog {...props} {...options} navigate={navigate} />
		));
}

function AddToLibraryDialog(
	props: UseDialogProps & AddToLibraryOptions & { navigate: NavigateFunction },
) {
	const dialog = useDialog(props);
	const { navigate } = props;
	const [step, setStep] = useState<Step>(
		props.target ? { kind: "setup", target: props.target } : { kind: "pick" },
	);
	const close = () => dialogManager.setState(props.id, { open: false });

	const pickCategory = (category: StorageCategory) => {
		if (category === "local") setStep({ kind: "folder" });
		else if (category === "external") setStep({ kind: "drive" });
		else if (category === "cloud") setStep({ kind: "cloud-provider" });
		else setStep({ kind: "network" });
	};

	if (step.kind === "setup") {
		return (
			<SetupDialog
				dialog={dialog}
				target={step.target}
				onBack={step.from ? () => setStep(step.from!) : undefined}
				onRetarget={(target) => setStep({ ...step, target })}
				onAdded={(result) => setStep({ kind: "done", target: step.target, result })}
			/>
		);
	}

	if (step.kind === "done") {
		return (
			<DoneDialog
				dialog={dialog}
				target={step.target}
				result={step.result}
				onOpen={
					props.onAdded
						? () => {
								close();
								props.onAdded?.({
									Physical: {
										device_slug: "local",
										path: step.result?.root ?? targetPath(step.target),
									},
								});
							}
						: undefined
				}
				onProgress={() => {
					close();
					navigate("/jobs");
				}}
			/>
		);
	}

	if (step.kind === "cloud") {
		return (
			<CloudDialog
				dialog={dialog}
				provider={step.provider}
				onBack={() => setStep({ kind: "cloud-provider" })}
				onConnected={(root) => {
					close();
					props.onAdded?.(root);
				}}
			/>
		);
	}

	const picker: Record<
		Exclude<Step["kind"], "setup" | "done" | "cloud">,
		{ title: string; description: string; icon: ReactNode; body: ReactNode; back?: Step }
	> = {
		pick: {
			title: "Add to Library",
			description: "Choose what to add",
			icon: <Books size={20} weight="fill" />,
			body: <CategoryGrid onSelect={pickCategory} />,
		},
		folder: {
			title: "Add a Folder",
			description: "Choose a folder on this computer",
			icon: <img src={FolderIcon} className="size-5" alt="" />,
			body: (
				<FolderPicker
					onPick={(path) =>
						setStep({ kind: "setup", target: { kind: "path", path }, from: step })
					}
				/>
			),
			back: { kind: "pick" },
		},
		drive: {
			title: "Add a Drive",
			description: "Choose a connected drive to add whole",
			icon: <HardDrive size={20} weight="fill" />,
			body: (
				<DrivePicker
					onPick={(volume) =>
						setStep({ kind: "setup", target: { kind: "volume", volume }, from: step })
					}
				/>
			),
			back: { kind: "pick" },
		},
		network: {
			title: "Network Protocol",
			description: "Choose a network file protocol",
			icon: <img src={ServerIcon} className="size-5" alt="" />,
			body: <NetworkComingSoon />,
			back: { kind: "pick" },
		},
		"cloud-provider": {
			title: "Cloud Storage",
			description: "Choose your cloud storage service",
			icon: cloudStepIcon,
			body: (
				<CloudProviderGrid
					onSelect={(provider) => setStep({ kind: "cloud", provider })}
				/>
			),
			back: { kind: "pick" },
		},
	};
	const current = picker[step.kind];

	return (
		<Shell
			dialog={dialog}
			title={current.title}
			description={current.description}
			icon={current.icon}
			hideButtons
			onBack={current.back ? () => setStep(current.back!) : undefined}
		>
			{current.body}
		</Shell>
	);
}

function Shell({
	dialog,
	title,
	description,
	icon,
	hideButtons,
	onBack,
	children,
	...rest
}: {
	dialog: ReturnType<typeof useDialog>;
	title: string;
	description?: ReactNode;
	icon: ReactNode;
	hideButtons?: boolean;
	onBack?: () => void;
	children: ReactNode;
	onSubmit?: () => void;
	ctaLabel?: string;
	ctaDanger?: boolean;
	cancelLabel?: string;
	submitDisabled?: boolean;
	loading?: boolean;
	buttonsSideContent?: ReactNode;
}) {
	const form = useForm();
	const { onSubmit, buttonsSideContent, ...dialogProps } = rest;
	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={title}
			description={description}
			icon={icon}
			hideButtons={hideButtons}
			onCancelled={true}
			onSubmit={onSubmit ? form.handleSubmit(onSubmit) : undefined}
			formClassName="!min-w-[520px] !max-w-[520px] max-h-[85vh] flex flex-col"
			buttonsSideContent={
				onBack || buttonsSideContent ? (
					<div className="flex items-center gap-2">
						{onBack && (
							<Button type="button" variant="gray" size="sm" onClick={onBack}>
								Back
							</Button>
						)}
						{buttonsSideContent}
					</div>
				) : undefined
			}
			{...dialogProps}
		>
			{children}
		</Dialog>
	);
}

function CloudDialog({
	dialog,
	provider,
	onBack,
	onConnected,
}: {
	dialog: ReturnType<typeof useDialog>;
	provider: CloudProvider;
	onBack: () => void;
	onConnected: (root: SdPath) => void;
}) {
	const { form, submit, pending } = useCloudConnection(provider, onConnected);
	return (
		<Dialog
			dialog={dialog}
			form={form}
			onSubmit={submit}
			title={`Connect ${provider.name}`}
			icon={<img src={provider.icon} className="size-5" alt="" />}
			description="Configure your cloud storage connection"
			ctaLabel="Connect"
			loading={pending}
			onCancelled={true}
			formClassName="!min-w-[520px] !max-w-[520px] max-h-[85vh] flex flex-col"
			buttonsSideContent={
				<Button type="button" variant="gray" size="sm" onClick={onBack}>
					Back
				</Button>
			}
		>
			<CloudConnectionFields provider={provider} form={form} />
		</Dialog>
	);
}

interface RiskWarning {
	validation: ValidateSourcePathOutput;
	wholeVolume: Volume | undefined;
}

function SetupDialog({
	dialog,
	target,
	onBack,
	onRetarget,
	onAdded,
}: {
	dialog: ReturnType<typeof useDialog>;
	target: AddTarget;
	onBack?: () => void;
	onRetarget: (target: AddTarget) => void;
	onAdded: (result: TrackSourceOutput | null) => void;
}) {
	const path = targetPath(target);
	const { data: config } = useLibraryQuery({ type: "config.library.get", input: null });
	const { data: sources } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});
	const { data: volumeList } = useLibraryQuery({
		type: "volumes.list",
		input: { filter: "All" },
	});
	const validatePath = useLibraryQuery(
		{
			type: "sources.validate_path",
			input: { path: { Physical: { device_slug: "local", path } } },
		},
		{ enabled: target.kind === "path" },
	);
	const trackSource = useLibraryMutation("sources.track");
	const trackVolume = useLibraryMutation("volumes.track");
	const updateConfig = useLibraryMutation("config.library.update");

	const volumes = volumeList?.volumes ?? [];
	const volume =
		target.kind === "volume" ? target.volume : containingVolume(path, volumes);
	const inclusion = inclusionOf(path, sources ?? []);
	const library = libraryDefaults(config?.adding);
	const defaults = effectiveDefaults(library, target, volume, inclusion.exact);
	const choices = placementChoices(volume, inclusion.exact);
	const loaded = Boolean(config && sources && volumeList);
	// A drive that is unmounted or locked cannot be added until it is back;
	// the core would refuse, and the modal says why before the attempt.
	const refusal = addRefusal(target, volumes);

	const [edits, setEdits] = useState<Partial<AddSettings>>({});
	const [typedName, setTypedName] = useState<string | null>(null);
	const [saveAsDefaults, setSaveAsDefaults] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);
	const [pending, setPending] = useState(false);

	const chosen: AddSettings = { ...defaults, ...edits };
	const changed = changedKeys(defaults, chosen);
	const name = typedName ?? inclusion.exact?.name ?? suggestedName(target);
	const set = <K extends keyof AddSettings>(key: K, value: AddSettings[K]) =>
		setEdits((previous) => ({ ...previous, [key]: value }));

	// A risky root (a home directory, a whole system drive) is said before
	// the add, with the whole drive offered where the core suggests it.
	const risk: RiskWarning | null =
		target.kind === "path" && validatePath.data && validatePath.data.risk_level !== "low"
			? {
					validation: validatePath.data,
					wholeVolume: validatePath.data.suggested_alternative
						? volumes.find(
								(candidate) =>
									candidate.fingerprint ===
									validatePath.data?.suggested_alternative?.volume_fingerprint,
							)
						: undefined,
				}
			: null;

	const submit = async () => {
		setFailure(null);
		setPending(true);
		try {
			// Only the user's own changes become defaults: what the scope
			// forced (an external drive's capture, a placement the drive
			// cannot hold) is this add's circumstance, not a library choice.
			const overrides = overridesFrom(defaults, chosen);
			if (saveAsDefaults && changed.length > 0) {
				await updateConfig.mutateAsync({ adding: overrides });
			}
			const payload = buildPayload(target, name, overrides);
			if (payload.type === "volumes.track") {
				const output = await trackVolume.mutateAsync(payload.input);
				onAdded(output.source);
			} else {
				onAdded(await trackSource.mutateAsync(payload.input));
			}
		} catch (error) {
			setFailure(describeError(error));
		} finally {
			setPending(false);
		}
	};

	const scopeLabel =
		target.kind === "volume" ? `Entire drive ${target.volume.name}` : path;
	const onSourceRoot = `${path.replace(/\/+$/, "")}/.spacedrive/`;

	return (
		<Shell
			dialog={dialog}
			title={inclusion.exact ? "Re-add to Library" : "Add to Library"}
			description={scopeLabel}
			icon={
				target.kind === "volume" ? (
					<img src={HDDIcon} className="size-5" alt="" />
				) : (
					<img src={FolderIcon} className="size-5" alt="" />
				)
			}
			onBack={onBack}
			onSubmit={() => void submit()}
			ctaLabel={risk ? "Add anyway" : inclusion.exact ? "Re-add to Library" : "Add to Library"}
			ctaDanger={Boolean(risk)}
			submitDisabled={!loaded || pending || Boolean(refusal)}
			loading={pending}
		>
			<div className="space-y-4 py-1">
				<IncludeCard target={target} volume={volume} />

				{refusal && (
					<Notice tone="warning">
						<span className="font-medium text-ink">Cannot add this now.</span>{" "}
						{refusal}
					</Notice>
				)}

				{inclusion.exact && (
					<Notice tone="info">
						<span className="font-medium text-ink">
							Already in this library as {inclusion.exact.name}.
						</span>{" "}
						Adding it again reopens its catalog
						{inclusion.exact.store_path ? ` at ${inclusion.exact.store_path}` : ""}{" "}
						and walks the folder for changes. The settings below are the ones it
						was saved with; capture can widen here but never narrows.
					</Notice>
				)}
				{!inclusion.exact && inclusion.enclosing && (
					<Notice tone="info">
						<span className="font-medium text-ink">
							Inside {inclusion.enclosing.name}
						</span>{" "}
						({inclusion.enclosing.root}), whose catalog already covers this folder.
						Adding it makes a nested source with its own catalog and settings.
					</Notice>
				)}
				{inclusion.nested.length > 0 && (
					<Notice tone="info">
						Contains{" "}
						{inclusion.nested.length === 1
							? `the source ${inclusion.nested[0]?.name}`
							: `${inclusion.nested.length} sources (${inclusion.nested
									.map((source) => source.name)
									.join(", ")})`}
						. They stay as they are; this add does not merge them.
					</Notice>
				)}

				{risk && (
					<RiskNotice
						risk={risk}
						onWholeVolume={
							risk.wholeVolume
								? () => onRetarget({ kind: "volume", volume: risk.wholeVolume! })
								: undefined
						}
					/>
				)}

				<div className="space-y-1.5">
					<label className="text-xs font-medium text-ink-dull" htmlFor="add-name">
						Name
					</label>
					<Input
						id="add-name"
						size="sm"
						value={name}
						onChange={(event) => setTypedName(event.target.value)}
						placeholder={suggestedName(target)}
					/>
				</div>

				<div className="space-y-1.5">
					<div className="flex items-center justify-between">
						<div className="text-xs font-medium text-ink-dull">Settings for this add</div>
						{changed.length > 0 ? (
							<button
								type="button"
								className="flex items-center gap-1 text-xs text-accent hover:underline"
								onClick={() => setEdits({})}
							>
								<ArrowCounterClockwise size={12} weight="bold" />
								Reset to defaults
							</button>
						) : (
							<span className="text-xs text-ink-faint">
								{inclusion.exact ? "As saved" : "Library defaults"}
							</span>
						)}
					</div>
					<div className="divide-y divide-app-line overflow-hidden rounded-lg border border-app-line bg-app-box">
						<Row
							label="Store catalog"
							changed={changed.includes("placement")}
							description={
								chosen.placement === "on_source"
									? `Under ${onSourceRoot}, so the catalog travels with the drive.`
									: "In the library on this device, under the Spacedrive data folder."
							}
						>
							<PlacementPicker
								value={chosen.placement}
								choices={choices}
								onChange={(placement) => set("placement", placement)}
							/>
						</Row>
						<Row
							label="Keep an offline copy"
							changed={changed.includes("keep_offline_copy")}
							muted={chosen.placement !== "on_source"}
							description={
								chosen.placement === "on_source"
									? "A replica in the library keeps the catalog answering while the drive is unplugged. Uses extra space; does not copy the files."
									: "Only applies when the catalog lives on the source."
							}
						>
							<Switch
								size="sm"
								checked={chosen.keep_offline_copy}
								disabled={chosen.placement !== "on_source"}
								onCheckedChange={(value) => set("keep_offline_copy", value)}
							/>
						</Row>
						<Row
							label="Capture everything"
							changed={changed.includes("unfiltered")}
							description={
								chosen.unfiltered
									? "Records every readable file, including system files, .git and dev directories."
									: "Skips system files, .git and dev directories, as Indexer settings say."
							}
						>
							<Switch
								size="sm"
								checked={chosen.unfiltered}
								disabled={Boolean(inclusion.exact?.settings?.unfiltered)}
								onCheckedChange={(value) => set("unfiltered", value)}
							/>
						</Row>
						<Row
							label="Identify content"
							changed={changed.includes("identify_content")}
							description="Hashes each file once the walk lands, so duplicates and integrity checks work."
						>
							<Switch
								size="sm"
								checked={chosen.identify_content}
								onCheckedChange={(value) => set("identify_content", value)}
							/>
						</Row>
					</div>
				</div>

				{!inclusion.exact && (
					<RadixCheckbox
						checked={saveAsDefaults}
						onCheckedChange={(value) => setSaveAsDefaults(value === true)}
						label="Use these changes as the library's defaults for later adds"
						labelClassName="text-xs text-ink-dull"
					/>
				)}

				{failure && <Failure message={failure} />}
			</div>
		</Shell>
	);
}

function IncludeCard({ target, volume }: { target: AddTarget; volume: Volume | undefined }) {
	const drive = volume
		? `${volume.name}${volume.mount_type === "Network" ? " (network)" : volume.mount_type === "External" ? " (external)" : ""}`
		: "Drive not matched";
	return (
		<div className="flex items-center gap-3 rounded-lg border border-app-line bg-app-box px-3 py-2.5">
			<img
				src={target.kind === "volume" ? HDDIcon : FolderIcon}
				className="size-9"
				alt=""
			/>
			<div className="min-w-0 flex-1">
				<div className="truncate text-sm font-medium text-ink">
					{target.kind === "volume" ? target.volume.name : suggestedName(target)}
				</div>
				<div className="truncate text-xs text-ink-faint">
					{target.kind === "volume" ? "Whole drive" : "Folder"} · {drive} · this
					device
				</div>
			</div>
		</div>
	);
}

function Row({
	label,
	description,
	changed,
	muted,
	children,
}: {
	label: string;
	description: string;
	changed: boolean;
	muted?: boolean;
	children: ReactNode;
}) {
	return (
		<div className={clsx("flex items-center justify-between gap-4 px-3 py-2.5", muted && "opacity-60")}>
			<div className="flex min-w-0 flex-1 flex-col gap-0.5">
				<div className="flex items-center gap-2">
					<span className="text-sm text-ink">{label}</span>
					{changed && (
						<span className="rounded-full bg-accent/15 px-1.5 py-px text-[10px] font-medium text-accent">
							changed
						</span>
					)}
				</div>
				<span className="text-tiny leading-snug text-ink-faint">{description}</span>
			</div>
			<div className="flex shrink-0 items-center">{children}</div>
		</div>
	);
}

const PLACEMENTS: { value: StorePlacement; label: string }[] = [
	{ value: "in_library", label: "In library" },
	{ value: "on_source", label: "On source" },
];

function PlacementPicker({
	value,
	choices,
	onChange,
}: {
	value: StorePlacement;
	choices: ReturnType<typeof placementChoices>;
	onChange: (value: StorePlacement) => void;
}) {
	const unavailable = PLACEMENTS.find((option) => !choices[option.value].available);
	return (
		<div className="flex flex-col items-end gap-1">
			<div className="flex rounded-md bg-app-input p-0.5">
				{PLACEMENTS.map((option) => {
					const choice = choices[option.value];
					return (
						<button
							key={option.value}
							type="button"
							disabled={!choice.available}
							title={choice.reason}
							onClick={() => onChange(option.value)}
							className={clsx(
								"rounded px-2 py-1 text-xs font-medium transition-colors",
								value === option.value
									? "bg-accent text-white"
									: "text-ink-dull hover:text-ink",
								!choice.available && "cursor-not-allowed opacity-40 hover:text-ink-dull",
							)}
						>
							{option.label}
						</button>
					);
				})}
			</div>
			{unavailable && (
				<span className="max-w-[220px] text-right text-[10px] leading-snug text-ink-faint">
					{unavailable.label} unavailable: {choices[unavailable.value].reason}
				</span>
			)}
		</div>
	);
}

function Notice({ tone, children }: { tone: "info" | "warning"; children: ReactNode }) {
	return (
		<div
			className={clsx(
				"rounded-lg border px-3 py-2 text-xs leading-relaxed text-ink-dull",
				tone === "warning"
					? "border-yellow-500/40 bg-yellow-500/10"
					: "border-accent/20 bg-accent/10",
			)}
		>
			{children}
		</div>
	);
}

function RiskNotice({
	risk,
	onWholeVolume,
}: {
	risk: RiskWarning;
	onWholeVolume?: () => void;
}) {
	return (
		<Notice tone="warning">
			<div className="flex items-start gap-2">
				<Warning size={16} weight="fill" className="mt-px shrink-0 text-yellow-500" />
				<div className="flex-1 space-y-1.5">
					<div className="font-medium text-ink">
						{risk.validation.risk_level === "high"
							? "This is a lot to add"
							: "Check this path"}
					</div>
					{risk.validation.warnings.map((warning, index) => (
						<div key={index}>
							<div>{warning.message}</div>
							{warning.suggestion && (
								<div className="text-ink-faint">{warning.suggestion}</div>
							)}
						</div>
					))}
					{risk.validation.suggested_alternative && (
						<div className="pt-1">
							<div>{risk.validation.suggested_alternative.message}</div>
							{onWholeVolume && (
								<Button
									type="button"
									variant="outline"
									size="sm"
									className="mt-1.5"
									onClick={onWholeVolume}
								>
									Add the whole drive {risk.validation.suggested_alternative.volume_name}{" "}
									instead
								</Button>
							)}
						</div>
					)}
				</div>
			</div>
		</Notice>
	);
}

function DoneDialog({
	dialog,
	target,
	result,
	onOpen,
	onProgress,
}: {
	dialog: ReturnType<typeof useDialog>;
	target: AddTarget;
	result: TrackSourceOutput | null;
	onOpen?: () => void;
	onProgress: () => void;
}) {
	const settings = result?.settings;
	const facts: { label: string; value: string }[] = result
		? [
				{ label: "Root", value: result.root },
				{
					label: "Catalog",
					value: result.store_path
						? result.store_path
						: settings?.placement === "on_source"
							? "On the drive"
							: "In the library",
				},
				{
					label: "Identity",
					value: result.catalog_reused
						? "Catalog reused; continuing where it left off"
						: "New catalog",
				},
				{
					label: "Capture",
					value: settings?.unfiltered ? "Everything" : "Filtered",
				},
				{
					label: "Identify content",
					value: settings?.identify_content === false ? "Off" : "On",
				},
			]
		: [];
	if (result && settings?.placement === "on_source") {
		facts.push({
			label: "Offline copy",
			value: settings.keep_offline_copy === false ? "Off" : "On",
		});
	}

	return (
		<Shell
			dialog={dialog}
			title={result ? `Added ${result.name}` : "Drive tracked"}
			description={result?.root ?? targetPath(target)}
			icon={<CheckCircle size={20} weight="fill" className="text-green-500" />}
			onSubmit={onProgress}
			ctaLabel={result?.job_id ? "View progress" : "View jobs"}
			cancelLabel="Close"
			buttonsSideContent={
				onOpen ? (
					<Button type="button" variant="gray" size="sm" onClick={onOpen}>
						Open
					</Button>
				) : undefined
			}
		>
			<div className="space-y-3 py-1">
				{result ? (
					<>
						<div className="divide-y divide-app-line overflow-hidden rounded-lg border border-app-line bg-app-box">
							{facts.map((fact) => (
								<div key={fact.label} className="flex items-baseline gap-3 px-3 py-2">
									<span className="w-28 shrink-0 text-xs text-ink-faint">{fact.label}</span>
									<span className="min-w-0 flex-1 truncate text-xs text-ink" title={fact.value}>
										{fact.value}
									</span>
								</div>
							))}
						</div>
						<p className="text-xs text-ink-dull">
							{result.job_id
								? "The walk is running; files appear as it goes and content identification follows."
								: "No walk was started. Open Jobs to see what is running."}
						</p>
					</>
				) : (
					<p className="text-xs text-ink-dull">
						The drive is tracked but offline, so no source was set up. Its files are
						walked when it comes back.
					</p>
				)}
			</div>
		</Shell>
	);
}

export type { AddTarget };
