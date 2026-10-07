import { useState } from "react";
import { useForm, useWatch } from "react-hook-form";
import {
	Folder,
	FolderOpen,
	HardDrive,
	CloudArrowUp,
} from "@phosphor-icons/react";
import {
	Button,
	Input,
	Label,
	Dialog,
	dialogManager,
	useDialog,
	CircleButton,
} from "@spacedrive/primitives";
import type {
	VolumeAddCloudInput,
	CloudServiceType,
	CloudStorageConfig,
	SdPath,
	ValidateSourcePathInput,
	ValidateSourcePathOutput,
	RiskLevel,
	ValidationWarning as PathValidationWarning,
	WholeVolumeSuggestion,
} from "@sd/ts-client";
import { useLibraryMutation, useLibraryQuery, useSpacedriveClient } from "../../../contexts/SpacedriveContext";
import { usePlatform } from "../../../contexts/PlatformContext";
import clsx from "clsx";

// Import icons
import FolderIcon from "@sd/assets/icons/Folder.webp";
import DriveIcon from "@sd/assets/icons/Drive.webp";
import HDDIcon from "@sd/assets/icons/HDD.webp";
import ServerIcon from "@sd/assets/icons/Server.webp";
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.webp";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.webp";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.webp";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.webp";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.webp";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.webp";
import DriveDAV from "@sd/assets/icons/Drive-DAV.webp";
import DriveBox from "@sd/assets/icons/Drive-Box.webp";

type StorageCategory = "local" | "cloud" | "network" | "external";
type ModalStep = "category" | "provider" | "local-config" | "cloud-config";

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

interface LocalFolderFormData {
	path: string;
	name: string;
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

interface StorageDialogProps {
	dialog: ReturnType<typeof useDialog>;
	form: any;
	title: string;
	icon: React.ReactNode;
	description: React.ReactNode;
	onSubmit?: any;
	ctaLabel?: string;
	ctaDanger?: boolean;
	submitDisabled?: boolean;
	loading?: boolean;
	showBackButton?: boolean;
	onBack?: () => void;
	hideButtons?: boolean;
	children: React.ReactNode;
}

function StorageDialog({
	dialog,
	form,
	title,
	icon,
	description,
	onSubmit,
	ctaLabel,
	ctaDanger,
	submitDisabled,
	loading,
	showBackButton,
	onBack,
	hideButtons,
	children,
}: StorageDialogProps) {
	return (
		<Dialog
			dialog={dialog}
			form={form}
			onSubmit={onSubmit}
			title={title}
			icon={icon}
			description={description}
			ctaLabel={ctaLabel}
			ctaDanger={ctaDanger}
			submitDisabled={submitDisabled}
			onCancelled={true}
			loading={loading}
			formClassName="!min-w-[480px] !max-w-[480px] max-h-[80vh] flex flex-col"
			hideButtons={hideButtons}
			buttonsSideContent={
				showBackButton ? (
					<Button variant="gray" size="sm" onClick={onBack}>
						Back
					</Button>
				) : undefined
			}
		>
			{children}
		</Dialog>
	);
}

export function useAddStorageDialog(
	onStorageAdded?: (sdPath: any) => void,
	initialPath?: string,
) {
	return dialogManager.create((props) => (
		<AddStorageDialog
			{...props}
			onStorageAdded={onStorageAdded}
			initialPath={initialPath}
		/>
	));
}

function AddStorageDialog(props: {
	id: number;
	onStorageAdded?: (sdPath: any) => void;
	initialPath?: string;
}) {
	const dialog = useDialog(props);
	const platform = usePlatform();

	// Derive initial folder name from path
	const initialFolderName =
		props.initialPath?.split("/").filter(Boolean).pop() || "";

	const [step, setStep] = useState<ModalStep>(
		props.initialPath ? "local-config" : "category",
	);
	const [selectedCategory, setSelectedCategory] =
		useState<StorageCategory | null>(props.initialPath ? "local" : null);
	const [selectedProvider, setSelectedProvider] =
		useState<CloudProvider | null>(null);
	const [validationResult, setValidationResult] = useState<{
		riskLevel: RiskLevel;
		warnings: PathValidationWarning[];
		suggestion: WholeVolumeSuggestion | null;
	} | null>(null);

	console.log("AddStorageDialog render:", { validationResult, step });

	const client = useSpacedriveClient();
	const trackSource = useLibraryMutation("sources.track");
	const addCloudVolume = useLibraryMutation("volumes.add_cloud");
	const trackVolume = useLibraryMutation("volumes.track");
	const indexVolume = useLibraryMutation("volumes.index");
	const { data: systemFolders } = useLibraryQuery({
		type: "paths.system_folders",
		input: null,
	});
	const { data: sourcesData } = useLibraryQuery({
		type: "sources.list",
		input: { data_type: null },
	});
	// A folder already inside a local source is kept; suggesting it would
	// only nest a second source under the first.
	const trackedRoots = (sourcesData ? [...sourcesData] : [])
		.filter((source) => !source.device_id && source.root)
		.map((source) => source.root as string);
	const suggestedFolders = (systemFolders?.folders ?? []).filter((folder) => {
		if (!("Physical" in folder.sd_path)) return true;
		const path = folder.sd_path.Physical.path;
		return !trackedRoots.some(
			(root) => path === root || path.startsWith(`${root}/`),
		);
	});
	const { data: volumesData } = useLibraryQuery({
		type: "volumes.list",
		input: { filter: "UntrackedOnly" },
	});

	const volumes = volumesData?.volumes || [];

	const localForm = useForm<LocalFolderFormData>({
		defaultValues: {
			path: props.initialPath || "",
			name: initialFolderName,
		},
	});

	const cloudForm = useForm<CloudFormData>({
		defaultValues: {
			display_name: "",
		},
	});

	// Dummy form for non-form dialogs (to satisfy Dialog component)
	const dummyForm = useForm();

	// Subscribe to the local form's `path` field so the picker step's
	// Continue button reacts as the user types into the input.
	const localPathTyped =
		useWatch({ control: localForm.control, name: "path" }) || "";

	const handleCategorySelect = (category: StorageCategory) => {
		setSelectedCategory(category);
		setStep("provider");
	};

	const handleProviderSelect = (provider: CloudProvider) => {
		setSelectedProvider(provider);
		setStep("cloud-config");
	};

	const handleBack = () => {
		if (step === "cloud-config") {
			setStep("provider");
			setSelectedProvider(null);
		} else if (step === "local-config") {
			setStep("provider");
			localForm.setValue("path", "");
			localForm.setValue("name", "");
		} else {
			setStep("category");
			setSelectedCategory(null);
			setSelectedProvider(null);
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

	const onSubmitManualPath = localForm.handleSubmit((data) => {
		const path = (data.path || "").trim();
		if (!path) return;
		if (!data.name) {
			const folderName = path.split("/").filter(Boolean).pop() || path;
			localForm.setValue("name", folderName);
		}
		setStep("local-config");
	});

	const handleVolumeSelect = async (volume: any) => {
		try {
			// Tracking a volume indexes it, so there is no second step.
			const result = await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				display_name: volume.display_name || volume.name,
			});
			dialog.state.open = false;

			if (volume.mount_point && props.onStorageAdded) {
				props.onStorageAdded(volume.mount_point);
			}
			return result;
		} catch (error) {
			console.error("Failed to track volume:", error);
		}
	};

	const onSubmitLocal = localForm.handleSubmit(async (data) => {
		// Validate path first
		const validateInput: ValidateSourcePathInput = {
			path: {
				Physical: {
					device_slug: "local",
					path: data.path,
				},
			},
		};

		let validation: ValidateSourcePathOutput | undefined;
		try {
			validation = await client.execute<
				ValidateSourcePathInput,
				ValidateSourcePathOutput
			>("query:sources.validate_path", validateInput);
		} catch (error) {
			console.error("Failed to validate path:", error);
			// Continue anyway if validation fails
		}

		// Show warning if path is risky (but don't block submission if user already saw it)
		if (validation && (validation.risk_level === "medium" || validation.risk_level === "high") && !validationResult) {
			console.log("Validation warning for risk level:", validation.risk_level);
			setValidationResult({
				riskLevel: validation.risk_level,
				warnings: validation.warnings,
				suggestion: validation.suggested_alternative || null,
			});
			// Don't submit yet - let user review the warning
			return;
		}

		// Path is safe or user proceeding anyway - clear validation and add the source
		if (validationResult) {
			setValidationResult(null);
		}

		try {
			const result = await trackSource.mutateAsync({
				path: data.path,
				name: data.name || null,
			});
			dialog.state.open = false;

			if (props.onStorageAdded) {
				const root: SdPath = {
					Physical: { device_slug: "local", path: result.root },
				};
				props.onStorageAdded(root);
			}
		} catch (error) {
			console.error("Failed to add source:", error);
			localForm.setError("root", {
				type: "manual",
				message:
					error instanceof Error
						? error.message
						: "Failed to add source",
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
			await addCloudVolume.mutateAsync(volumeInput);

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

			dialog.state.open = false;

			if (props.onStorageAdded) {
				const root: SdPath = {
					Cloud: {
						service: provider.cloudServiceType,
						identifier: cloudIdentifier,
						path: "",
					},
				};
				props.onStorageAdded(root);
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

	const handleUseVolumeIndexing = async () => {
		if (!validationResult?.suggestion) return;

		try {
			const result = await indexVolume.mutateAsync({
				fingerprint: validationResult.suggestion.volume_fingerprint,
				scope: "Recursive",
			});

			// Clear validation and close dialog
			setValidationResult(null);
			dialog.state.open = false;

			console.log("Volume indexed:", result.message);
		} catch (error) {
			console.error("Failed to index volume:", error);
			localForm.setError("root", {
				type: "manual",
				message:
					error instanceof Error
						? error.message
						: "Failed to index volume",
			});
		}
	};

	// Render category selection
	if (step === "category") {
		return (
			<StorageDialog
				dialog={dialog}
				form={dummyForm}
				title="Add Storage"
				icon={<CloudArrowUp size={20} weight="fill" />}
				description="Choose the type of storage you want to connect"
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
							<img
								src={category.icon}
								className="size-12"
								alt=""
							/>
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
			</StorageDialog>
		);
	}

	// Render provider selection for cloud
	if (step === "provider" && selectedCategory === "cloud") {
		return (
			<StorageDialog
				dialog={dialog}
				form={dummyForm}
				title="Select Cloud Provider"
				icon={<CloudArrowUp size={20} weight="fill" />}
				description="Choose your cloud storage service"
				hideButtons={true}
				showBackButton={true}
				onBack={handleBack}
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
							<img
								src={provider.icon}
								className="size-10"
								alt=""
							/>
							<div className="text-xs font-medium text-ink text-center">
								{provider.name}
							</div>
						</button>
					))}
				</div>
			</StorageDialog>
		);
	}

	// Render provider selection for network
	if (step === "provider" && selectedCategory === "network") {
		return (
			<StorageDialog
				dialog={dialog}
				form={dummyForm}
				title="Select Network Protocol"
				icon={<img src={ServerIcon} className="size-5" alt="" />}
				description="Choose your network file protocol"
				hideButtons={true}
				showBackButton={true}
				onBack={handleBack}
			>
				<div className="space-y-3">
					<div className="rounded-lg bg-accent/10 border border-accent/20 p-4 text-sm text-ink">
						<strong>Coming Soon</strong>
						<p className="mt-1 text-ink-dull">
							Network protocol support (SMB, NFS, SFTP, WebDAV) is
							currently in development. Check back in a future
							update!
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
								<img
									src={protocol.icon}
									className="size-8"
									alt=""
								/>
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
			</StorageDialog>
		);
	}

	// Render provider selection for external
	if (step === "provider" && selectedCategory === "external") {
		return (
			<StorageDialog
				dialog={dialog}
				form={dummyForm}
				title="Track External Drive"
				icon={<HardDrive size={20} weight="fill" />}
				description="Select a connected drive to track"
				hideButtons={true}
				showBackButton={true}
				onBack={handleBack}
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
									<img
										src={HDDIcon}
										className="size-8"
										alt=""
									/>
									<div className="flex-1 min-w-0">
										<div className="text-sm font-medium text-ink truncate">
											{volume.display_name || volume.name}
										</div>
										<div className="text-xs text-ink-faint">
											{volume.mount_point} •{" "}
											{typeof volume.file_system === 'string' ? volume.file_system : (volume.file_system as any)?.Other ?? 'Unknown'}
										</div>
									</div>
									<div className="text-xs text-ink-dull">
										{volume.total_capacity
											? (
													volume.total_capacity / 1e9
												).toFixed(0)
											: "?"}{" "}
										GB
									</div>
								</button>
							))}
						</div>
					) : (
						<div className="rounded-lg bg-app-box border border-app-line p-6 text-center">
							<p className="text-sm text-ink-dull">
								No untracked external drives found. Connect a
								drive and refresh to see it here.
							</p>
						</div>
					)}
				</div>
			</StorageDialog>
		);
	}

	// Render local folder configuration (browse + suggested + settings)
	if (step === "provider" && selectedCategory === "local") {
		return (
			<StorageDialog
				dialog={dialog}
				form={localForm}
				onSubmit={onSubmitManualPath}
				title="Add Local Folder"
				icon={<Folder size={20} weight="fill" />}
				description="Choose a folder to index and manage"
				ctaLabel="Continue"
				submitDisabled={!localPathTyped.trim()}
				showBackButton={true}
				onBack={handleBack}
			>
				<div className="space-y-4 flex flex-col">
					<div className="space-y-2">
						<Label>Browse</Label>
						<div className="relative">
							<Input
								value={localPathTyped}
								onChange={(e) =>
									localForm.setValue("path", e.target.value, {
										shouldValidate: true,
										shouldDirty: true,
									})
								}
								placeholder="Type or paste a folder path"
								size="lg"
								className="pr-14"
							/>
							<CircleButton
								icon={FolderOpen}
								onClick={handleBrowse}
								className="absolute right-2 top-1/2 -translate-y-1/2"
							/>
						</div>
					</div>

					{suggestedFolders.length > 0 && (
							<div className="space-y-2">
								<Label>Suggested Folders</Label>
								<div className="grid grid-cols-2 gap-2 max-h-[280px] overflow-y-auto pr-1">
									{suggestedFolders.map((folder) => (
										<button
											key={folder.path}
											type="button"
											onClick={() =>
												handleSelectSuggested(
													folder.path,
													folder.name,
												)
											}
											className="flex items-center gap-3 rounded-lg border border-app-line bg-app-box p-3 text-left transition-all hover:bg-app-hover hover:border-accent/50 h-fit"
										>
											<Folder
												className="size-5 shrink-0 text-accent"
												weight="fill"
											/>
											<div className="min-w-0 flex-1">
												<div className="text-sm font-medium text-ink truncate">
													{folder.name}
												</div>
												<div className="text-xs text-ink-faint truncate">
													{folder.path}
												</div>
											</div>
										</button>
									))}
								</div>
							</div>
						)}
				</div>
			</StorageDialog>
		);
	}

	// Render local folder settings (after path selected)
	if (step === "local-config") {
		return (
			<StorageDialog
				dialog={dialog}
				form={localForm}
				onSubmit={onSubmitLocal}
				title="Add to Library"
				icon={<Folder size={20} weight="fill" />}
				description={localForm.watch("path")}
				ctaLabel={validationResult ? "Proceed Anyway" : "Add to Library"}
				ctaDanger={!!validationResult}
				loading={trackSource.isPending}
				showBackButton={true}
				onBack={handleBack}
			>
				<div className="space-y-4">
					{/* Validation Warning Card */}
					{validationResult && (
						<div className={clsx(
							"rounded-lg border p-4 space-y-3",
							validationResult.riskLevel === "high"
								? "bg-red-500/10 border-red-500/50"
								: "bg-yellow-500/10 border-yellow-500/50"
						)}>
							<div className="flex items-start gap-2">
								<span className="text-lg">
									{validationResult.riskLevel === "high" ? "⚠️" : "⚡"}
								</span>
								<div className="flex-1 space-y-2">
									<p className="text-sm font-medium text-ink">
										{validationResult.riskLevel === "high"
											? "High Risk Path Detected"
											: "Warning"}
									</p>
									{validationResult.warnings.map((warning, i) => (
										<div key={i} className="space-y-1">
											<p className="text-xs text-ink-dull">{warning.message}</p>
											{warning.suggestion && (
												<p className="text-xs text-ink-faint italic">
													💡 {warning.suggestion}
												</p>
											)}
										</div>
									))}

									{validationResult.suggestion && (
										<div className="mt-3 pt-3 border-t border-app-line/50">
											<p className="text-xs font-medium text-ink mb-2">
												Alternative Suggestion
											</p>
											<p className="text-xs text-ink-dull mb-2">
												{validationResult.suggestion.message}
											</p>
											<Button
												onClick={handleUseVolumeIndexing}
												variant="accent"
												size="sm"
												className="w-full"
											>
												Index Volume: {validationResult.suggestion.volume_name}
											</Button>
										</div>
									)}
								</div>
							</div>
						</div>
					)}

					<div className="space-y-2">
						<Label slug="name">Display Name</Label>
						<Input
							{...localForm.register("name")}
							size="md"
							placeholder="My Documents"
							className="bg-app-input"
						/>
					</div>


					{localForm.formState.errors.root && (
						<p className="text-xs text-red-500">
							{localForm.formState.errors.root.message}
						</p>
					)}
				</div>
			</StorageDialog>
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
			<StorageDialog
				dialog={dialog}
				form={cloudForm}
				onSubmit={onSubmitCloud}
				title={`Add ${provider.name}`}
				icon={<img src={provider.icon} className="size-5" alt="" />}
				description="Configure your cloud storage connection"
				ctaLabel="Add Storage"
				loading={addCloudVolume.isPending}
				showBackButton={true}
				onBack={handleBack}
			>
				<div className="space-y-4 h-full overflow-y-auto pr-1">
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
										{provider.id === "r2" &&
											" (e.g., https://account.r2.cloudflarestorage.com)"}
										{provider.id === "minio" &&
											" (e.g., http://localhost:9000)"}
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
			</StorageDialog>
		);
	}

	return null;
}