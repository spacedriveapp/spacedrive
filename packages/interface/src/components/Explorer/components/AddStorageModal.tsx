import { useState } from "react";
import { useForm } from "react-hook-form";
import {
	Folder,
	FolderOpen,
	HardDrive,
	CloudArrowUp,
	ArrowLeft,
} from "@phosphor-icons/react";
import {
	Button,
	Input,
	Label,
	Dialog,
	dialogManager,
	useDialog,
	TopBarButton,
} from "@sd/ui";
import { Tabs } from "@sd/ui";
import type {
	IndexMode,
	LocationAddInput,
	VolumeAddCloudInput,
	CloudServiceType,
	CloudStorageConfig,
} from "@sd/ts-client";
import { useLibraryMutation, useLibraryQuery } from "../../../context";
import { usePlatform } from "../../../platform";
import clsx from "clsx";

// Import icons
import FolderIcon from "@sd/assets/icons/Folder.png";
import DriveIcon from "@sd/assets/icons/Drive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.png";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.png";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.png";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.png";
import DriveMega from "@sd/assets/icons/Drive-Mega.png";
import DriveDAV from "@sd/assets/icons/Drive-DAV.png";
import DriveBox from "@sd/assets/icons/Drive-Box.png";

type StorageCategory = "local" | "cloud" | "network" | "external";
type ModalStep = "category" | "provider" | "local-config" | "cloud-config";
type SettingsTab = "preset" | "jobs";

interface CategoryOption {
	id: StorageCategory;
	label: string;
	description: string;
	icon: string;
}

interface CloudProvider {
	id: CloudServiceType | "r2" | "minio";
	name: string;
	icon: string;
	cloudServiceType: CloudServiceType; // Actual type for API
}

interface NetworkProtocol {
	id: string;
	name: string;
	description: string;
	icon: string;
}

interface JobOption {
	id: string;
	label: string;
	description: string;
	presets: IndexMode[];
	order: number;
}

interface LocalFolderFormData {
	path: string;
	name: string;
	mode: IndexMode;
}

interface CloudFormData {
	display_name: string;
	// S3 fields
	bucket?: string;
	region?: string;
	access_key_id?: string;
	secret_access_key?: string;
	endpoint?: string;
	// OAuth fields
	access_token?: string;
	refresh_token?: string;
	client_id?: string;
	client_secret?: string;
	root?: string;
	// Azure fields
	container?: string;
	account_name?: string;
	account_key?: string;
	// GCS fields
	credential?: string;
}

const categories: CategoryOption[] = [
	{
		id: "local",
		label: "Local Folder",
		description: "Index a folder on your computer",
		icon: FolderIcon,
	},
	{
		id: "cloud",
		label: "Cloud Storage",
		description: "Connect S3, Google Drive, Dropbox, etc.",
		icon: DriveIcon,
	},
	{
		id: "network",
		label: "Network Protocol",
		description: "SMB, NFS, SFTP, WebDAV",
		icon: ServerIcon,
	},
	{
		id: "external",
		label: "External Drive",
		description: "Track a connected drive",
		icon: HDDIcon,
	},
];

const cloudProviders: CloudProvider[] = [
	{
		id: "s3",
		name: "Amazon S3",
		icon: DriveAmazonS3,
		cloudServiceType: "s3",
	},
	{
		id: "r2",
		name: "Cloudflare R2",
		icon: DriveAmazonS3,
		cloudServiceType: "s3",
	},
	{
		id: "minio",
		name: "MinIO",
		icon: DriveAmazonS3,
		cloudServiceType: "s3",
	},
	{
		id: "b2",
		name: "Backblaze B2",
		icon: DriveBackBlaze,
		cloudServiceType: "b2",
	},
	{
		id: "wasabi",
		name: "Wasabi",
		icon: DriveAmazonS3,
		cloudServiceType: "wasabi",
	},
	{
		id: "spaces",
		name: "DO Spaces",
		icon: DriveAmazonS3,
		cloudServiceType: "spaces",
	},
	{
		id: "gdrive",
		name: "Google Drive",
		icon: DriveGoogleDrive,
		cloudServiceType: "gdrive",
	},
	{
		id: "dropbox",
		name: "Dropbox",
		icon: DriveDropbox,
		cloudServiceType: "dropbox",
	},
	{
		id: "onedrive",
		name: "OneDrive",
		icon: DriveOneDrive,
		cloudServiceType: "onedrive",
	},
	{
		id: "gcs",
		name: "Google Cloud",
		icon: DriveGoogleDrive,
		cloudServiceType: "gcs",
	},
	{
		id: "azblob",
		name: "Azure Blob",
		icon: DriveBox,
		cloudServiceType: "azblob",
	},
	{
		id: "cloud",
		name: "pCloud",
		icon: DrivePCloud,
		cloudServiceType: "cloud",
	},
];

const networkProtocols: NetworkProtocol[] = [
	{
		id: "smb",
		name: "SMB / CIFS",
		description: "Windows file sharing",
		icon: ServerIcon,
	},
	{
		id: "nfs",
		name: "NFS",
		description: "Unix/Linux network file system",
		icon: ServerIcon,
	},
	{
		id: "sftp",
		name: "SFTP",
		description: "SSH file transfer protocol",
		icon: ServerIcon,
	},
	{
		id: "webdav",
		name: "WebDAV",
		description: "Web-based file access",
		icon: DriveDAV,
	},
];

const indexModes: Array<{
	value: IndexMode;
	label: string;
	description: string;
}> = [
	{
		value: "Shallow",
		label: "Shallow",
		description: "Just filesystem metadata",
	},
	{
		value: "Content",
		label: "Content",
		description: "Generate content identities",
	},
	{
		value: "Deep",
		label: "Deep",
		description: "Full indexing + thumbnails",
	},
];

const jobOptions: JobOption[] = [
	{
		id: "thumbnail",
		label: "Generate Thumbnails",
		description: "Create preview thumbnails for images and videos",
		presets: ["Content", "Deep"],
		order: 1,
	},
	{
		id: "thumbstrip",
		label: "Generate Thumbstrips",
		description: "Create video storyboard grids (5×5 grid of frames)",
		presets: ["Deep"],
		order: 2,
	},
	{
		id: "proxy",
		label: "Generate Proxies",
		description: "Create scrubbing proxies for videos (~8s per video)",
		presets: [],
		order: 3,
	},
	{
		id: "ocr",
		label: "Extract Text (OCR)",
		description: "OCR and text extraction from images/PDFs",
		presets: [],
		order: 4,
	},
	{
		id: "speech_to_text",
		label: "Speech to Text",
		description: "Transcribe audio and video files",
		presets: [],
		order: 5,
	},
];

export function useAddStorageDialog(
	onStorageAdded?: (id: string) => void,
) {
	return dialogManager.create((props) => (
		<AddStorageDialog {...props} onStorageAdded={onStorageAdded} />
	));
}

function AddStorageDialog(props: {
	id: number;
	onStorageAdded?: (id: string) => void;
}) {
	const dialog = useDialog(props);
	const platform = usePlatform();

	const [step, setStep] = useState<ModalStep>("category");
	const [selectedCategory, setSelectedCategory] =
		useState<StorageCategory | null>(null);
	const [selectedProvider, setSelectedProvider] =
		useState<CloudProvider | null>(null);
	const [selectedProtocol, setSelectedProtocol] =
		useState<NetworkProtocol | null>(null);
	const [tab, setTab] = useState<SettingsTab>("preset");

	const addLocation = useLibraryMutation("locations.add");
	const addCloudVolume = useLibraryMutation("volumes.add_cloud");
	const trackVolume = useLibraryMutation("volumes.track");
	const { data: suggestedLocations } = useLibraryQuery({
		type: "locations.suggested",
		input: null,
	});
	const { data: volumesData } = useLibraryQuery({
		type: "volumes.list",
		input: { filter: "UntrackedOnly" },
	});

	const volumes = volumesData?.volumes || [];

	const localForm = useForm<LocalFolderFormData>({
		defaultValues: {
			path: "",
			name: "",
			mode: "Deep",
		},
	});

	const cloudForm = useForm<CloudFormData>({
		defaultValues: {
			display_name: "",
		},
	});

	// Dummy form for non-form dialogs (to satisfy Dialog component)
	const dummyForm = useForm();

	// Update selected jobs when preset mode changes
	const currentMode = localForm.watch("mode");
	const [selectedJobs, setSelectedJobs] = useState<Set<string>>(
		new Set(
			jobOptions.filter((j) => j.presets.includes("Deep")).map((j) => j.id),
		),
	);

	// Sync selected jobs with preset when mode changes
	const handleModeChange = (mode: IndexMode) => {
		localForm.setValue("mode", mode);
		const presetJobs = jobOptions.filter((j) => j.presets.includes(mode));
		setSelectedJobs(new Set(presetJobs.map((j) => j.id)));
	};

	const toggleJob = (jobId: string) => {
		setSelectedJobs((prev) => {
			const next = new Set(prev);
			if (next.has(jobId)) {
				next.delete(jobId);
			} else {
				next.add(jobId);
			}
			return next;
		});
	};

	const handleCategorySelect = (category: StorageCategory) => {
		setSelectedCategory(category);
		if (category === "local") {
			setStep("provider"); // Will show local folder UI
		} else if (category === "cloud") {
			setStep("provider");
		} else if (category === "network") {
			setStep("provider");
		} else if (category === "external") {
			setStep("provider");
		}
	};

	const handleProviderSelect = (provider: CloudProvider) => {
		setSelectedProvider(provider);
		setStep("cloud-config");
	};

	const handleBack = () => {
		if (step === "provider" || step === "local-config" || step === "cloud-config") {
			setStep("category");
			setSelectedCategory(null);
			setSelectedProvider(null);
			setSelectedProtocol(null);
		} else if (step === "cloud-config") {
			setStep("provider");
		}
	};

	const handleBrowse = async () => {
		if (!platform.openDirectoryPickerDialog) {
			console.error("Directory picker not available on this platform");
			return;
		}

		const selected = await platform.openDirectoryPickerDialog({
			title: "Choose a folder to add",
			multiple: false,
		});

		if (selected && typeof selected === "string") {
			localForm.setValue("path", selected);
			const folderName = selected.split("/").pop() || "";
			localForm.setValue("name", folderName);
			setStep("local-config");
		}
	};

	const handleSelectSuggested = (path: string, name: string) => {
		localForm.setValue("path", path);
		localForm.setValue("name", name);
		setStep("local-config");
	};

	const handleVolumeSelect = async (volume: any) => {
		try {
			// Step 1: Track the volume
			const trackResult = await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				display_name: volume.name,
			});

			// Step 2: Create a location for the volume's mount point
			const locationInput: LocationAddInput = {
				path: {
					Physical: {
						device_slug: "local",
						path: volume.mount_point || "/",
					},
				},
				name: volume.name,
				mode: "Deep",
				job_policies: {},
			};

			const locationResult = await addLocation.mutateAsync(locationInput);
			dialog.state.open = false;

			if (locationResult?.id && props.onStorageAdded) {
				props.onStorageAdded(locationResult.id);
			}
		} catch (error) {
			console.error("Failed to track volume and add location:", error);
		}
	};

	const onSubmitLocal = localForm.handleSubmit(async (data) => {
		const job_policies: any = {};
		selectedJobs.forEach((jobId) => {
			job_policies[jobId] = { enabled: true };
		});

		const input: LocationAddInput = {
			path: {
				Physical: {
					device_slug: "local",
					path: data.path,
				},
			},
			name: data.name || null,
			mode: data.mode,
			job_policies,
		};

		try {
			const result = await addLocation.mutateAsync(input);
			dialog.state.open = false;

			if (result?.id && props.onStorageAdded) {
				props.onStorageAdded(result.id);
			}
		} catch (error) {
			console.error("Failed to add location:", error);
			localForm.setError("root", {
				type: "manual",
				message:
					error instanceof Error ? error.message : "Failed to add location",
			});
		}
	});

	const onSubmitCloud = cloudForm.handleSubmit(async (data) => {
		if (!selectedProvider) return;

		let config: CloudStorageConfig;
		const provider = selectedProvider;

		// Build config based on provider type
		if (
			provider.cloudServiceType === "s3" ||
			provider.cloudServiceType === "b2" ||
			provider.cloudServiceType === "wasabi" ||
			provider.cloudServiceType === "spaces"
		) {
			config = {
				type: "S3",
				bucket: data.bucket!,
				region: data.region!,
				access_key_id: data.access_key_id!,
				secret_access_key: data.secret_access_key!,
				endpoint: data.endpoint || null,
			};
		} else if (
			provider.cloudServiceType === "gdrive" ||
			provider.cloudServiceType === "dropbox" ||
			provider.cloudServiceType === "onedrive"
		) {
			const configType =
				provider.cloudServiceType === "gdrive"
					? "GoogleDrive"
					: provider.cloudServiceType === "dropbox"
						? "Dropbox"
						: "OneDrive";
			config = {
				type: configType as any,
				root: data.root || null,
				access_token: data.access_token!,
				refresh_token: data.refresh_token!,
				client_id: data.client_id!,
				client_secret: data.client_secret!,
			};
		} else if (provider.cloudServiceType === "azblob") {
			config = {
				type: "AzureBlob",
				container: data.container!,
				endpoint: data.endpoint || null,
				account_name: data.account_name!,
				account_key: data.account_key!,
			};
		} else if (provider.cloudServiceType === "gcs") {
			config = {
				type: "GoogleCloudStorage",
				bucket: data.bucket!,
				root: data.root || null,
				endpoint: data.endpoint || null,
				credential: data.credential!,
			};
		} else {
			throw new Error("Unsupported cloud provider");
		}

		const volumeInput: VolumeAddCloudInput = {
			service: provider.cloudServiceType,
			display_name: data.display_name,
			config,
		};

		try {
			// Step 1: Add the cloud volume and get fingerprint
			const volumeResult = await addCloudVolume.mutateAsync(volumeInput);

			// Determine the cloud identifier based on provider type
			let cloudIdentifier: string;
			if (
				provider.cloudServiceType === "s3" ||
				provider.cloudServiceType === "b2" ||
				provider.cloudServiceType === "wasabi" ||
				provider.cloudServiceType === "spaces"
			) {
				cloudIdentifier = data.bucket!;
			} else if (provider.cloudServiceType === "azblob") {
				cloudIdentifier = data.container!;
			} else if (provider.cloudServiceType === "gcs") {
				cloudIdentifier = data.bucket!;
			} else if (
				provider.cloudServiceType === "gdrive" ||
				provider.cloudServiceType === "dropbox" ||
				provider.cloudServiceType === "onedrive"
			) {
				cloudIdentifier = data.root || "root";
			} else {
				cloudIdentifier = "root";
			}

			// Step 2: Create a location for the cloud volume so it gets indexed
			const locationInput: LocationAddInput = {
				path: {
					Cloud: {
						service: provider.cloudServiceType,
						identifier: cloudIdentifier,
						path: "",
					},
				},
				name: data.display_name,
				mode: "Deep",
				job_policies: {},
			};

			const locationResult = await addLocation.mutateAsync(locationInput);
			dialog.state.open = false;

			if (locationResult?.id && props.onStorageAdded) {
				props.onStorageAdded(locationResult.id);
			}
		} catch (error) {
			console.error("Failed to add cloud storage:", error);
			cloudForm.setError("root", {
				type: "manual",
				message:
					error instanceof Error
						? error.message
						: "Failed to add cloud storage",
			});
		}
	});

	// Render category selection
	if (step === "category") {
		return (
			<Dialog
				dialog={dialog}
				form={dummyForm}
				title="Add Storage"
				icon={<CloudArrowUp size={20} weight="fill" />}
				description="Choose the type of storage you want to connect"
				className="w-[640px]"
				onCancelled={true}
				hideButtons={true}
			>
				<div className="grid grid-cols-2 gap-3">
					{categories.map((category) => (
						<button
							key={category.id}
							type="button"
							onClick={() => handleCategorySelect(category.id)}
							className={clsx(
								"flex flex-col items-center gap-3 rounded-lg border p-6",
								"transition-all hover:scale-[1.02]",
								"border-app-line bg-app-box hover:bg-app-hover hover:border-accent/50",
							)}
						>
							<img src={category.icon} className="size-12" alt="" />
							<div className="text-center">
								<div className="text-sm font-medium text-ink">
									{category.label}
								</div>
								<div className="mt-1 text-xs text-ink-faint">
									{category.description}
								</div>
							</div>
						</button>
					))}
				</div>
			</Dialog>
		);
	}

	// Render provider selection for cloud
	if (step === "provider" && selectedCategory === "cloud") {
		return (
			<Dialog
				dialog={dialog}
				form={dummyForm}
				title="Select Cloud Provider"
				icon={<CloudArrowUp size={20} weight="fill" />}
				description="Choose your cloud storage service"
				className="w-[640px]"
				onCancelled={true}
				hideButtons={true}
				buttonsSideContent={
					<Button variant="gray" size="sm" onClick={handleBack}>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="grid grid-cols-3 gap-3 max-h-[400px] overflow-y-auto pr-1">
					{cloudProviders.map((provider) => (
						<button
							key={provider.id}
							type="button"
							onClick={() => handleProviderSelect(provider)}
							className={clsx(
								"flex flex-col items-center gap-2 rounded-lg border p-4",
								"transition-all hover:scale-[1.02]",
								"border-app-line bg-app-box hover:bg-app-hover hover:border-accent/50",
							)}
						>
							<img src={provider.icon} className="size-10" alt="" />
							<div className="text-xs font-medium text-ink text-center">
								{provider.name}
							</div>
						</button>
					))}
				</div>
			</Dialog>
		);
	}

	// Render provider selection for network
	if (step === "provider" && selectedCategory === "network") {
		return (
			<Dialog
				dialog={dialog}
				form={dummyForm}
				title="Select Network Protocol"
				icon={<img src={ServerIcon} className="size-5" alt="" />}
				description="Choose your network file protocol"
				className="w-[640px]"
				onCancelled={true}
				hideButtons={true}
				buttonsSideContent={
					<Button variant="gray" size="sm" onClick={handleBack}>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="space-y-3">
					<div className="rounded-lg bg-accent/10 border border-accent/20 p-4 text-sm text-ink">
						<strong>Coming Soon</strong>
						<p className="mt-1 text-ink-dull">
							Network protocol support (SMB, NFS, SFTP, WebDAV) is currently in
							development. Check back in a future update!
						</p>
					</div>
					<div className="grid grid-cols-2 gap-3 opacity-50 pointer-events-none">
						{networkProtocols.map((protocol) => (
							<button
								key={protocol.id}
								type="button"
								disabled
								className={clsx(
									"flex items-center gap-3 rounded-lg border p-4",
									"border-app-line bg-app-box",
								)}
							>
								<img src={protocol.icon} className="size-8" alt="" />
								<div className="text-left">
									<div className="text-sm font-medium text-ink">
										{protocol.name}
									</div>
									<div className="text-xs text-ink-faint">
										{protocol.description}
									</div>
								</div>
							</button>
						))}
					</div>
				</div>
			</Dialog>
		);
	}

	// Render provider selection for external
	if (step === "provider" && selectedCategory === "external") {
		return (
			<Dialog
				dialog={dialog}
				form={dummyForm}
				title="Track External Drive"
				icon={<HardDrive size={20} weight="fill" />}
				description="Select a connected drive to track"
				className="w-[640px]"
				onCancelled={true}
				hideButtons={true}
				buttonsSideContent={
					<Button variant="gray" size="sm" onClick={handleBack}>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="space-y-3">
					{volumes && volumes.length > 0 ? (
						<div className="space-y-2 max-h-[400px] overflow-y-auto pr-1">
							{volumes.map((volume) => (
								<button
									key={volume.fingerprint}
									type="button"
									onClick={() => handleVolumeSelect(volume)}
									className={clsx(
										"w-full flex items-center gap-3 rounded-lg border p-3 text-left",
										"transition-all hover:scale-[1.01]",
										"border-app-line bg-app-box hover:bg-app-hover hover:border-accent/50",
									)}
								>
									<img src={HDDIcon} className="size-8" alt="" />
									<div className="flex-1 min-w-0">
										<div className="text-sm font-medium text-ink truncate">
											{volume.name}
										</div>
										<div className="text-xs text-ink-faint">
											{volume.mount_point} • {volume.filesystem}
										</div>
									</div>
									<div className="text-xs text-ink-dull">
										{volume.total_capacity ? (volume.total_capacity / 1e9).toFixed(0) : '?'} GB
									</div>
								</button>
							))}
						</div>
					) : (
						<div className="rounded-lg bg-app-box border border-app-line p-6 text-center">
							<p className="text-sm text-ink-dull">
								No untracked external drives found. Connect a drive and refresh
								to see it here.
							</p>
						</div>
					)}
				</div>
			</Dialog>
		);
	}

	// Render local folder configuration (browse + suggested + settings)
	if (step === "provider" && selectedCategory === "local") {
		return (
			<Dialog
				dialog={dialog}
				form={dummyForm}
				title="Add Local Folder"
				icon={<Folder size={20} weight="fill" />}
				description="Choose a folder to index and manage"
				className="w-[640px]"
				onCancelled={true}
				hideButtons={true}
				buttonsSideContent={
					<Button variant="gray" size="sm" onClick={handleBack}>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="space-y-4 flex flex-col">
					<div className="space-y-2">
						<Label>Browse</Label>
						<div className="relative">
							<Input
								value={localForm.watch("path") || ""}
								onChange={(e) => localForm.setValue("path", e.target.value)}
								placeholder="Select a custom folder"
								size="lg"
								className="pr-14"
							/>
							<TopBarButton
								icon={FolderOpen}
								onClick={handleBrowse}
								className="absolute right-2 top-1/2 -translate-y-1/2"
							/>
						</div>
					</div>

					{suggestedLocations && suggestedLocations.locations.length > 0 && (
						<div className="space-y-2">
							<Label>Suggested Locations</Label>
							<div className="grid grid-cols-2 gap-2 max-h-[280px] overflow-y-auto pr-1">
								{suggestedLocations.locations.map((loc) => (
									<button
										key={loc.path}
										type="button"
										onClick={() => handleSelectSuggested(loc.path, loc.name)}
										className="flex items-center gap-3 rounded-lg border border-app-line bg-app-box p-3 text-left transition-all hover:bg-app-hover hover:border-accent/50 h-fit"
									>
										<Folder
											className="size-5 shrink-0 text-accent"
											weight="fill"
										/>
										<div className="min-w-0 flex-1">
											<div className="text-sm font-medium text-ink truncate">
												{loc.name}
											</div>
											<div className="text-xs text-ink-faint truncate">
												{loc.path}
											</div>
										</div>
									</button>
								))}
							</div>
						</div>
					)}
				</div>
			</Dialog>
		);
	}

	// Render local folder settings (after path selected)
	if (step === "local-config") {
		return (
			<Dialog
				dialog={dialog}
				form={localForm}
				onSubmit={onSubmitLocal}
				title="Configure Location"
				icon={<Folder size={20} weight="fill" />}
				description={localForm.watch("path")}
				ctaLabel="Add Location"
				onCancelled={true}
				loading={addLocation.isPending}
				className="w-[640px]"
				buttonsSideContent={
					<Button
						variant="gray"
						size="sm"
						onClick={() => {
							setStep("provider");
							localForm.setValue("path", "");
							localForm.setValue("name", "");
						}}
					>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="space-y-4">
					<div className="space-y-2">
						<Label slug="name">Display Name</Label>
						<Input
							{...localForm.register("name")}
							size="md"
							placeholder="My Documents"
							className="bg-app-input"
						/>
					</div>

					<Tabs.Root value={tab} onValueChange={(v) => setTab(v as SettingsTab)}>
						<Tabs.List>
							<Tabs.Trigger value="preset">Preset</Tabs.Trigger>
							<Tabs.Trigger value="jobs">
								Jobs {selectedJobs.size > 0 && `(${selectedJobs.size})`}
							</Tabs.Trigger>
						</Tabs.List>

						<Tabs.Content value="preset" className="pt-3">
							<div className="space-y-2 max-h-[280px] overflow-y-auto pr-1">
								<Label>Indexing Mode</Label>
								<div className="grid grid-cols-3 gap-2">
									{indexModes.map((mode) => {
										const isSelected = currentMode === mode.value;
										return (
											<button
												key={mode.value}
												type="button"
												onClick={() => handleModeChange(mode.value)}
												className={clsx(
													"rounded-lg border p-3 text-left transition-all",
													isSelected
														? "border-accent bg-accent/5 ring-1 ring-accent"
														: "border-app-line bg-app-box hover:bg-app-hover",
												)}
											>
												<div className="text-xs font-medium text-ink">
													{mode.label}
												</div>
												<div className="mt-1 text-[11px] leading-tight text-ink-faint">
													{mode.description}
												</div>
											</button>
										);
									})}
								</div>
							</div>
						</Tabs.Content>

						<Tabs.Content value="jobs" className="pt-3">
							<div className="space-y-3 max-h-[280px] overflow-y-auto pr-1">
								<p className="text-xs text-ink-faint">
									Select which jobs to run after indexing. Extensions can add
									more jobs.
								</p>
								<div className="grid grid-cols-2 gap-2">
									{jobOptions.map((job) => {
										const isSelected = selectedJobs.has(job.id);
										return (
											<button
												key={job.id}
												type="button"
												onClick={() => toggleJob(job.id)}
												className={clsx(
													"flex items-start gap-2 rounded-lg border p-3 text-left transition-all",
													isSelected
														? "border-accent bg-accent/5 ring-1 ring-accent"
														: "border-app-line bg-app-box hover:bg-app-hover",
												)}
											>
												<div className="flex-1 min-w-0">
													<div className="text-xs font-medium text-ink">
														{job.label}
													</div>
													<div className="text-[11px] text-ink-faint mt-1 leading-tight">
														{job.description}
													</div>
												</div>
											</button>
										);
									})}
								</div>
							</div>
						</Tabs.Content>
					</Tabs.Root>

					{localForm.formState.errors.root && (
						<p className="text-xs text-red-500">
							{localForm.formState.errors.root.message}
						</p>
					)}
				</div>
			</Dialog>
		);
	}

	// Render cloud configuration form
	if (step === "cloud-config" && selectedProvider) {
		const provider = selectedProvider;
		const isS3Type =
			provider.cloudServiceType === "s3" ||
			provider.cloudServiceType === "b2" ||
			provider.cloudServiceType === "wasabi" ||
			provider.cloudServiceType === "spaces";
		const isOAuthType =
			provider.cloudServiceType === "gdrive" ||
			provider.cloudServiceType === "dropbox" ||
			provider.cloudServiceType === "onedrive";
		const isAzureType = provider.cloudServiceType === "azblob";
		const isGCSType = provider.cloudServiceType === "gcs";

		return (
			<Dialog
				dialog={dialog}
				form={cloudForm}
				onSubmit={onSubmitCloud}
				title={`Add ${provider.name}`}
				icon={<img src={provider.icon} className="size-5" alt="" />}
				description="Configure your cloud storage connection"
				ctaLabel="Add Storage"
				onCancelled={true}
				loading={addCloudVolume.isPending}
				className="w-[640px]"
				buttonsSideContent={
					<Button variant="gray" size="sm" onClick={() => setStep("provider")}>
						<ArrowLeft size={16} className="mr-1" />
						Back
					</Button>
				}
			>
				<div className="space-y-4 max-h-[400px] overflow-y-auto pr-1">
					<div className="space-y-2">
						<Label>Display Name</Label>
						<Input
							{...cloudForm.register("display_name")}
							size="md"
							placeholder={`My ${provider.name}`}
							className="bg-app-input"
						/>
					</div>

					{isS3Type && (
						<>
							<div className="space-y-2">
								<Label>Bucket</Label>
								<Input
									{...cloudForm.register("bucket")}
									size="md"
									placeholder="my-bucket"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Region</Label>
								<Input
									{...cloudForm.register("region")}
									size="md"
									placeholder="us-west-2"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Access Key ID</Label>
								<Input
									{...cloudForm.register("access_key_id")}
									size="md"
									placeholder="AKIA..."
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Secret Access Key</Label>
								<Input
									{...cloudForm.register("secret_access_key")}
									type="password"
									size="md"
									placeholder="••••••••••••••••••"
									className="bg-app-input"
								/>
							</div>
							{(provider.id === "r2" ||
								provider.id === "minio" ||
								provider.id === "wasabi" ||
								provider.id === "spaces") && (
								<div className="space-y-2">
									<Label>
										Endpoint
										{provider.id === "r2" && " (e.g., https://account.r2.cloudflarestorage.com)"}
										{provider.id === "minio" && " (e.g., http://localhost:9000)"}
									</Label>
									<Input
										{...cloudForm.register("endpoint")}
										size="md"
										placeholder={
											provider.id === "r2"
												? "https://account.r2.cloudflarestorage.com"
												: provider.id === "minio"
													? "http://localhost:9000"
													: "https://..."
										}
										className="bg-app-input"
									/>
								</div>
							)}
						</>
					)}

					{isOAuthType && (
						<>
							<div className="space-y-2">
								<Label>Client ID</Label>
								<Input
									{...cloudForm.register("client_id")}
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Client Secret</Label>
								<Input
									{...cloudForm.register("client_secret")}
									type="password"
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Access Token</Label>
								<Input
									{...cloudForm.register("access_token")}
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Refresh Token</Label>
								<Input
									{...cloudForm.register("refresh_token")}
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Root Path (Optional)</Label>
								<Input
									{...cloudForm.register("root")}
									size="md"
									placeholder="/"
									className="bg-app-input"
								/>
							</div>
						</>
					)}

					{isAzureType && (
						<>
							<div className="space-y-2">
								<Label>Container</Label>
								<Input
									{...cloudForm.register("container")}
									size="md"
									placeholder="my-container"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Account Name</Label>
								<Input
									{...cloudForm.register("account_name")}
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Account Key</Label>
								<Input
									{...cloudForm.register("account_key")}
									type="password"
									size="md"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Endpoint (Optional)</Label>
								<Input
									{...cloudForm.register("endpoint")}
									size="md"
									placeholder="https://..."
									className="bg-app-input"
								/>
							</div>
						</>
					)}

					{isGCSType && (
						<>
							<div className="space-y-2">
								<Label>Bucket</Label>
								<Input
									{...cloudForm.register("bucket")}
									size="md"
									placeholder="my-gcs-bucket"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Service Account Credential (JSON)</Label>
								<textarea
									{...cloudForm.register("credential")}
									rows={6}
									placeholder='{"type": "service_account", ...}'
									className="w-full rounded-lg border border-app-line bg-app-input px-3 py-2 text-sm text-ink font-mono"
								/>
							</div>
							<div className="space-y-2">
								<Label>Root Path (Optional)</Label>
								<Input
									{...cloudForm.register("root")}
									size="md"
									placeholder="/"
									className="bg-app-input"
								/>
							</div>
							<div className="space-y-2">
								<Label>Endpoint (Optional)</Label>
								<Input
									{...cloudForm.register("endpoint")}
									size="md"
									placeholder="https://storage.googleapis.com"
									className="bg-app-input"
								/>
							</div>
						</>
					)}

					{cloudForm.formState.errors.root && (
						<p className="text-xs text-red-500">
							{cloudForm.formState.errors.root.message}
						</p>
					)}
				</div>
			</Dialog>
		);
	}

	return null;
}
