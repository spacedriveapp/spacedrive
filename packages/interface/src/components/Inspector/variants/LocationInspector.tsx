import {
	Info,
	Gear,
	Briefcase,
	ClockCounterClockwise,
	HardDrive,
	DotsThree,
	Hash,
	Sparkle,
	Image,
	MagnifyingGlass,
	Trash,
	FunnelX,
	ToggleLeft,
	ToggleRight,
	X,
	Play,
	FilmStrip,
	VideoCamera,
	FolderOpen,
	ArrowsClockwise,
} from "@phosphor-icons/react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate } from "react-router-dom";
import {
	InfoRow,
	Section,
	Divider,
	Tabs,
	TabContent,
} from "../Inspector";
import clsx from "clsx";
import type { Location } from "@sd/ts-client";
import { Button, Dialog, dialogManager, useDialog, TopBarButton, type UseDialogProps } from "@sd/ui";
import { useLibraryMutation } from "../../../contexts/SpacedriveContext";
import { useContextMenu } from "../../../hooks/useContextMenu";
import LocationIcon from "@sd/assets/icons/Location.png";

interface LocationInspectorProps {
	location: Location;
}

export function LocationInspector({ location }: LocationInspectorProps) {
	const [activeTab, setActiveTab] = useState("overview");

	const tabs = [
		{ id: "overview", label: "Overview", icon: Info },
		{ id: "indexing", label: "Indexing", icon: Gear },
		{ id: "jobs", label: "Jobs", icon: Briefcase },
		{ id: "activity", label: "Activity", icon: ClockCounterClockwise },
		{ id: "devices", label: "Devices", icon: HardDrive },
		{ id: "more", label: "More", icon: DotsThree },
	];

	return (
		<>
			{/* Tabs */}
			<Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

			{/* Tab Content */}
			<div className="flex-1 overflow-hidden flex flex-col mt-2.5">
				<TabContent id="overview" activeTab={activeTab}>
					<OverviewTab location={location} />
				</TabContent>

				<TabContent id="indexing" activeTab={activeTab}>
					<IndexingTab location={location} />
				</TabContent>

				<TabContent id="jobs" activeTab={activeTab}>
					<JobsTab location={location} />
				</TabContent>

				<TabContent id="activity" activeTab={activeTab}>
					<ActivityTab location={location} />
				</TabContent>

				<TabContent id="devices" activeTab={activeTab}>
					<DevicesTab location={location} />
				</TabContent>

				<TabContent id="more" activeTab={activeTab}>
					<MoreTab location={location} />
				</TabContent>
			</div>
		</>
	);
}

function OverviewTab({ location }: { location: Location }) {
	const rescanLocation = useLibraryMutation("locations.rescan");
	const routeLocation = useLocation();
	const navigate = useNavigate();
	const isOverview = routeLocation.pathname === '/';

	const reindexMenu = useContextMenu({
		items: [
			{
				icon: MagnifyingGlass,
				label: "Quick Reindex",
				onClick: () => {
					rescanLocation.mutate({
						location_id: location.id,
						full_rescan: false,
					});
				},
			},
			{
				icon: Sparkle,
				label: "Full Reindex",
				onClick: () => {
					rescanLocation.mutate({
						location_id: location.id,
						full_rescan: true,
					});
				},
			},
		],
	});

	const formatBytes = (bytes: number | null | undefined) => {
		if (!bytes || bytes === 0) return "0 B";
		const k = 1024;
		const sizes = ["B", "KB", "MB", "GB", "TB"];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
	};

	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	};

	const formatScanState = (scanState: any) => {
		if (!scanState) return "Unknown";
		if (scanState.Idle) return "Idle";
		if (scanState.Scanning) return `Scanning ${scanState.Scanning.progress}%`;
		if (scanState.Completed) return "Completed";
		if (scanState.Failed) return "Failed";
		return "Unknown";
	};

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
			{/* Location icon */}
			<div className="flex justify-center h-48 items-center w-full px-4">
				<img src={LocationIcon} className="size-24" alt="Location" />
			</div>

			{/* Location name */}
			<div className="px-2 text-center">
				<h4 className="text-sm font-semibold text-sidebar-ink truncate">
					{location.name || "Unnamed Location"}
				</h4>
				<p className="text-xs text-sidebar-inkDull mt-0.5">
					Local Storage
				</p>
			</div>

			<Divider />

			{/* Action Buttons */}
			<div className="px-2 mb-5 flex gap-2">
				{isOverview && (
					<TopBarButton
						icon={FolderOpen}
						onClick={() => {
							const encodedPath = encodeURIComponent(JSON.stringify(location.sd_path));
							navigate(`/explorer?path=${encodedPath}`);
						}}
						className="flex-1"
					>
						Open Location
					</TopBarButton>
				)}

				<TopBarButton
					icon={ArrowsClockwise}
					onClick={reindexMenu.show}
					title="Reindex location"
				/>
			</div>

			{/* Details */}
			<Section title="Details" icon={Info}>
				<InfoRow label="Path" value={location.path} mono />
			{location.total_file_count != null && (
				<InfoRow
					label="Total Files"
					value={location.total_file_count?.toLocaleString() ?? "0"}
				/>
			)}
				<InfoRow
					label="Total Size"
					value={formatBytes(location.total_byte_size)}
				/>
				<InfoRow label="Scan State" value={formatScanState(location.scan_state)} />
				{location.last_scan_at && (
					<InfoRow
						label="Last Scan"
						value={formatDate(location.last_scan_at)}
					/>
				)}
			</Section>

			{/* Index Mode */}
			<Section title="Index Mode" icon={Gear}>
				<InfoRow
					label="Mode"
					value={
						location.index_mode.charAt(0).toUpperCase() +
						location.index_mode.slice(1)
					}
				/>
			</Section>
		</div>
	);
}

function IndexingTab({ location }: { location: Location }) {
	const [indexMode, setIndexMode] = useState<"shallow" | "content" | "deep">(
		location.index_mode as "shallow" | "content" | "deep",
	);
	const [ignoreRules, setIgnoreRules] = useState([
		".git",
		"node_modules",
		"*.tmp",
		".DS_Store",
	]);

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<Section title="Index Mode" icon={Gear}>
				<p className="text-xs text-sidebar-inkDull mb-3">
					Controls how deeply this location is indexed
				</p>

				<div className="space-y-2">
					<RadioOption
						value="shallow"
						label="Shallow"
						description="Just filesystem metadata (fastest)"
						checked={indexMode === "shallow"}
						onChange={() => setIndexMode("shallow")}
					/>
					<RadioOption
						value="content"
						label="Content"
						description="Generate content identities"
						checked={indexMode === "content"}
						onChange={() => setIndexMode("content")}
					/>
					<RadioOption
						value="deep"
						label="Deep"
						description="Full indexing with thumbnails and text extraction"
						checked={indexMode === "deep"}
						onChange={() => setIndexMode("deep")}
					/>
				</div>
			</Section>

			<Section title="Ignore Rules" icon={FunnelX}>
				<p className="text-xs text-sidebar-inkDull mb-3">
					Files and folders matching these patterns will be ignored
				</p>

				<div className="space-y-1">
					{ignoreRules.map((pattern, i) => (
						<IgnoreRule
							key={i}
							pattern={pattern}
							onRemove={() => {
								setIgnoreRules(
									ignoreRules.filter((_, idx) => idx !== i),
								);
							}}
						/>
					))}
				</div>

				<button className="mt-2 text-xs text-accent hover:text-accent/80 transition-colors">
					+ Add Rule
				</button>
			</Section>
		</div>
	);
}

function JobsTab({ location }: { location: Location }) {
	const updateLocation = useLibraryMutation("locations.update");
	const triggerJob = useLibraryMutation("locations.triggerJob");

	const updatePolicy = async (
		updates: Partial<typeof location.job_policies>,
	) => {
		await updateLocation.mutateAsync({
			id: location.id,
			job_policies: {
				...location.job_policies,
				...updates,
			},
		});
	};

	const thumbnails = location.job_policies?.thumbnail?.enabled ?? true;
	const thumbstrips = location.job_policies?.thumbstrip?.enabled ?? true;
	const proxies = location.job_policies?.proxy?.enabled ?? false;
	const ocr = location.job_policies?.ocr?.enabled ?? false;
	const speech = location.job_policies?.speech_to_text?.enabled ?? false;

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Configure which processing jobs run automatically for this
				location
			</p>

			<Section title="Media Processing" icon={Image}>
				<div className="space-y-2.5">
					<JobConfigRow
						label="Generate Thumbnails"
						description="Create preview thumbnails for images and videos"
						enabled={thumbnails}
						onToggle={(enabled) =>
							updatePolicy({
								thumbnail: {
									...(location.job_policies?.thumbnail ?? {}),
									enabled,
								},
							})
						}
						onTrigger={() =>
							triggerJob.mutate({
								location_id: location.id,
								job_type: "thumbnail",
								force: false,
							})
						}
						isTriggering={triggerJob.isPending}
					/>
					<JobConfigRow
						label="Generate Thumbstrips"
						description="Create video storyboard grids (5×5 grid of frames)"
						enabled={thumbstrips}
						onToggle={(enabled) =>
							updatePolicy({
								thumbstrip: {
									...(location.job_policies?.thumbstrip ?? {}),
									enabled,
								},
							})
						}
						onTrigger={() =>
							triggerJob.mutate({
								location_id: location.id,
								job_type: "thumbstrip",
								force: false,
							})
						}
						isTriggering={triggerJob.isPending}
						icon={FilmStrip}
					/>
					<JobConfigRow
						label="Generate Proxies"
						description="Create scrubbing proxies for videos (180p @ 15fps)"
						enabled={proxies}
						onToggle={(enabled) =>
							updatePolicy({
								proxy: {
									...(location.job_policies?.proxy ?? {}),
									enabled,
								},
							})
						}
						onTrigger={() =>
							triggerJob.mutate({
								location_id: location.id,
								job_type: "proxy",
								force: false,
							})
						}
						isTriggering={triggerJob.isPending}
						icon={VideoCamera}
					/>
				</div>
			</Section>

			<Section title="AI Processing" icon={Sparkle}>
				<div className="space-y-2.5">
					<JobConfigRow
						label="Extract Text (OCR)"
						description="Scan images for text content"
						enabled={ocr}
						onToggle={(enabled) =>
							updatePolicy({
								ocr: { ...(location.job_policies?.ocr ?? {}), enabled },
							})
						}
						onTrigger={() =>
							triggerJob.mutate({
								location_id: location.id,
								job_type: "ocr",
								force: false,
							})
						}
						isTriggering={triggerJob.isPending}
					/>
					<JobConfigRow
						label="Speech to Text"
						description="Transcribe audio and video files"
						enabled={speech}
						onToggle={(enabled) =>
							updatePolicy({
								speech_to_text: {
									...(location.job_policies?.speech_to_text ?? {}),
									enabled,
								},
							})
						}
						onTrigger={() =>
							triggerJob.mutate({
								location_id: location.id,
								job_type: "speech_to_text",
								force: false,
							})
						}
						isTriggering={triggerJob.isPending}
					/>
				</div>
			</Section>
		</div>
	);
}

function ActivityTab({ location }: { location: Location }) {
	const activity = [
		{ action: "Full Scan Completed", time: "10 min ago", files: 12456 },
		{ action: "Thumbnails Generated", time: "1 hour ago", files: 234 },
		{ action: "Content Hashes Updated", time: "3 hours ago", files: 5678 },
		{ action: "Metadata Extracted", time: "5 hours ago", files: 890 },
		{ action: "Location Added", time: "Jan 15, 2025", files: 0 },
	];

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Recent indexing activity and job history
			</p>

			<div className="space-y-0.5">
				{activity.map((item, i) => (
					<div
						key={i}
						className="flex items-start gap-3 p-2 hover:bg-app-box/40 rounded-lg transition-colors"
					>
						<ClockCounterClockwise
							className="size-4 text-sidebar-inkDull shrink-0 mt-0.5"
							weight="bold"
						/>
						<div className="flex-1 min-w-0">
							<div className="text-xs text-sidebar-ink">
								{item.action}
							</div>
							<div className="text-[11px] text-sidebar-inkDull mt-0.5">
								{item.time}
								{item.files > 0 &&
									` · ${item.files.toLocaleString()} files`}
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

function DevicesTab({ location }: { location: Location }) {
	const devices = [
		{
			name: "MacBook Pro",
			status: "online" as const,
			lastSeen: "2 min ago",
		},
		{
			name: "Desktop PC",
			status: "offline" as const,
			lastSeen: "2 days ago",
		},
		{
			name: "Home Server",
			status: "online" as const,
			lastSeen: "5 min ago",
		},
	];

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-4 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<p className="text-xs text-sidebar-inkDull">
				Devices that have access to this location
			</p>

			<div className="space-y-2">
				{devices.map((device, i) => (
					<div
						key={i}
						className="p-2.5 bg-app-box/40 rounded-lg border border-app-line/50"
					>
						<div className="flex items-center gap-2">
							<HardDrive
								className="size-4 text-accent"
								weight="bold"
							/>
							<div className="flex-1 min-w-0">
								<div className="text-xs font-medium text-sidebar-ink">
									{device.name}
								</div>
								<div className="text-[11px] text-sidebar-inkDull flex items-center gap-1">
									<div
										className={clsx(
											"size-1.5 rounded-full",
											device.status === "online"
												? "bg-green-500"
												: "bg-sidebar-inkDull",
										)}
									/>
									<span>
										{device.status === "online"
											? "Online"
											: "Offline"}{" "}
										· {device.lastSeen}
									</span>
								</div>
							</div>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

interface DeleteLocationDialogProps extends UseDialogProps {
	locationId: number;
	locationName: string;
}

function useDeleteLocationDialog() {
	return (locationId: number, locationName: string) =>
		dialogManager.create((props: DeleteLocationDialogProps) => (
			<DeleteLocationDialog {...props} locationId={locationId} locationName={locationName} />
		));
}

function DeleteLocationDialog({ locationId, locationName, ...props }: DeleteLocationDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const queryClient = useQueryClient();
	const removeLocation = useLibraryMutation("locations.remove", {
		onSuccess: () => {
			// Manually invalidate the locations query until the backend emits ResourceDeleted events
			// This forces a refetch so the location disappears from the sidebar immediately
			queryClient.invalidateQueries({
				predicate: (query) => {
					const key = query.queryKey;
					return Array.isArray(key) && key[0] === "query:locations.list";
				},
			});

			// Close the dialog
			dialogManager.setState(dialog.id, { open: false });
		},
	});

	const handleDelete = async () => {
		try {
			await removeLocation.mutateAsync({
				location_id: String(locationId),
			});
		} catch (error) {
			console.error("Failed to remove location:", error);
		}
	};

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title="Remove Location"
			description={`Are you sure you want to remove "${locationName}"? Your files will not be deleted from disk.`}
			icon={<Trash className="text-red-400" weight="bold" />}
			ctaLabel="Remove Location"
			ctaDanger
			cancelLabel="Cancel"
			cancelBtn
			onSubmit={handleDelete}
			loading={removeLocation.isPending}
		/>
	);
}

function MoreTab({ location }: { location: Location }) {
	const openDeleteDialog = useDeleteLocationDialog();

	const formatDate = (dateStr: string) => {
		const date = new Date(dateStr);
		return date.toLocaleDateString("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	};

	return (
		<div className="no-scrollbar mask-fade-out flex flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10 px-2 pt-2">
			<Section title="Advanced" icon={Gear}>
				<InfoRow
					label="Location ID"
					value={String(location.id).slice(0, 8) + "..."}
					mono
				/>
				{location.created_at && (
					<InfoRow
						label="Created"
						value={formatDate(location.created_at)}
					/>
				)}
				{location.last_scan_at && (
					<InfoRow
						label="Last Scan"
						value={formatDate(location.last_scan_at)}
					/>
				)}
			</Section>

			<Section title="Danger Zone" icon={Trash}>
				<p className="text-xs text-sidebar-inkDull mb-3">
					Removing this location will not delete your files
				</p>
				<button
					onClick={() => openDeleteDialog(location.id, location.name)}
					className="w-full px-3 py-2 bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 rounded-lg text-sm font-medium text-red-400 transition-colors"
				>
					<div className="flex items-center justify-center gap-2">
						<Trash className="size-4" weight="bold" />
						<span>Remove Location</span>
					</div>
				</button>
			</Section>
		</div>
	);
}

// Helper Components

interface RadioOptionProps {
	value: string;
	label: string;
	description: string;
	checked: boolean;
	onChange: () => void;
}

function RadioOption({
	value,
	label,
	description,
	checked,
	onChange,
}: RadioOptionProps) {
	return (
		<button
			onClick={onChange}
			className={clsx(
				"w-full p-2.5 rounded-lg border transition-colors text-left",
				checked
					? "bg-accent/10 border-accent/30"
					: "bg-app-box/40 border-app-line/50 hover:bg-app-box/60",
			)}
		>
			<div className="flex items-start gap-2">
				<div
					className={clsx(
						"size-4 rounded-full border-2 shrink-0 mt-0.5 flex items-center justify-center",
						checked ? "border-accent" : "border-sidebar-inkDull",
					)}
				>
					{checked && (
						<div className="size-2 rounded-full bg-accent" />
					)}
				</div>
				<div className="flex-1 min-w-0">
					<div className="text-xs font-medium text-sidebar-ink">
						{label}
					</div>
					<div className="text-[11px] text-sidebar-inkDull mt-0.5">
						{description}
					</div>
				</div>
			</div>
		</button>
	);
}

interface IgnoreRuleProps {
	pattern: string;
	onRemove: () => void;
}

function IgnoreRule({ pattern, onRemove }: IgnoreRuleProps) {
	return (
		<div className="flex items-center gap-2 p-2 bg-app-box/40 rounded-lg border border-app-line/50 group">
			<code className="flex-1 text-xs text-sidebar-ink font-mono">
				{pattern}
			</code>
			<button
				onClick={onRemove}
				className="size-5 rounded flex items-center justify-center opacity-0 group-hover:opacity-100 hover:bg-red-500/20 transition-all"
				title="Remove rule"
			>
				<X className="size-3 text-red-400" weight="bold" />
			</button>
		</div>
	);
}

interface JobConfigRowProps {
	label: string;
	description: string;
	enabled: boolean;
	onToggle: (enabled: boolean) => void;
	onTrigger: () => void;
	isTriggering: boolean;
	icon?: React.ComponentType<any>;
}

function JobConfigRow({
	label,
	description,
	enabled,
	onToggle,
	onTrigger,
	isTriggering,
	icon: Icon,
}: JobConfigRowProps) {
	return (
		<div className="w-full p-3 bg-app-box/40 rounded-lg border border-app-line/50">
			{/* Header with toggle and icon */}
			<div className="space-y-1.5">
				<button
					onClick={() => onToggle(!enabled)}
					className="flex items-center gap-2.5 w-full text-left group"
				>
					{enabled ? (
						<ToggleRight
							className="size-5 text-accent shrink-0"
							weight="fill"
						/>
					) : (
						<ToggleLeft
							className="size-5 text-sidebar-inkDull shrink-0 group-hover:text-sidebar-ink transition-colors"
							weight="fill"
						/>
					)}
					<div className="flex items-center gap-2 flex-1 min-w-0">
						{Icon && (
							<Icon
								className="size-4 text-sidebar-inkDull shrink-0"
								weight="bold"
							/>
						)}
						<div className="flex-1 min-w-0">
							<div className="text-xs font-medium text-sidebar-ink">
								{label}
							</div>
						</div>
					</div>
				</button>

				{/* Description */}
				<p className="text-[11px] text-sidebar-inkDull leading-relaxed pl-7">
					{description}
				</p>
			</div>

			{/* Run button */}
			<Button
				onClick={onTrigger}
				disabled={!enabled || isTriggering}
				variant="gray"
				size="sm"
				className="w-full flex items-center justify-center gap-1.5 mt-2.5"
				title={enabled ? "Run job now" : "Enable job first"}
			>
				<Play className="size-3" weight="fill" />
				{isTriggering ? "Running..." : "Run Now"}
			</Button>
		</div>
	);
}