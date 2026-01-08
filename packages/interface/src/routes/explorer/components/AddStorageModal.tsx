import {
  CloudArrowUp,
  Folder,
  FolderOpen,
  HardDrive,
} from "@phosphor-icons/react";
import DriveIcon from "@sd/assets/icons/Drive.png";
import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.png";
import DriveBox from "@sd/assets/icons/Drive-Box.png";
import DriveDAV from "@sd/assets/icons/Drive-DAV.png";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.png";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.png";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.png";
// Import icons
import FolderIcon from "@sd/assets/icons/Folder.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import type {
  CloudServiceType,
  CloudStorageConfig,
  IndexMode,
  LocationAddInput,
  ValidationWarning as PathValidationWarning,
  RiskLevel,
  ValidateLocationPathInput,
  VolumeAddCloudInput,
  VolumeIndexingSuggestion,
} from "@sd/ts-client";
import {
  Button,
  Dialog,
  dialogManager,
  Input,
  Label,
  Tabs,
  TopBarButton,
  useDialog,
} from "@sd/ui";
import clsx from "clsx";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { usePlatform } from "../../../contexts/PlatformContext";
import {
  useLibraryMutation,
  useLibraryQuery,
  useSpacedriveClient,
} from "../../../contexts/SpacedriveContext";

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

interface StorageDialogProps {
  dialog: ReturnType<typeof useDialog>;
  form: any;
  title: string;
  icon: React.ReactNode;
  description: React.ReactNode;
  onSubmit?: any;
  ctaLabel?: string;
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
  loading,
  showBackButton,
  onBack,
  hideButtons,
  children,
}: StorageDialogProps) {
  return (
    <Dialog
      buttonsSideContent={
        showBackButton ? (
          <Button onClick={onBack} size="sm" variant="gray">
            Back
          </Button>
        ) : undefined
      }
      ctaLabel={ctaLabel}
      description={description}
      dialog={dialog}
      form={form}
      formClassName="!min-w-[480px] !max-w-[480px] max-h-[80vh] flex flex-col"
      hideButtons={hideButtons}
      icon={icon}
      loading={loading}
      onCancelled={true}
      onSubmit={onSubmit}
      title={title}
    >
      {children}
    </Dialog>
  );
}

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
  initialPath?: string
) {
  return dialogManager.create((props) => (
    <AddStorageDialog
      {...props}
      initialPath={initialPath}
      onStorageAdded={onStorageAdded}
    />
  ));
}

function AddStorageDialog(props: {
  id: number;
  onStorageAdded?: (id: string) => void;
  initialPath?: string;
}) {
  const dialog = useDialog(props);
  const platform = usePlatform();

  // Derive initial folder name from path
  const initialFolderName =
    props.initialPath?.split("/").filter(Boolean).pop() || "";

  const [step, setStep] = useState<ModalStep>(
    props.initialPath ? "local-config" : "category"
  );
  const [selectedCategory, setSelectedCategory] =
    useState<StorageCategory | null>(props.initialPath ? "local" : null);
  const [selectedProvider, setSelectedProvider] =
    useState<CloudProvider | null>(null);
  const [tab, setTab] = useState<SettingsTab>("preset");
  const [showWarning, setShowWarning] = useState(false);
  const [validationResult, setValidationResult] = useState<{
    riskLevel: RiskLevel;
    warnings: PathValidationWarning[];
    suggestion: VolumeIndexingSuggestion | null;
  } | null>(null);

  const client = useSpacedriveClient();
  const addLocation = useLibraryMutation("locations.add");
  const addCloudVolume = useLibraryMutation("volumes.add_cloud");
  const trackVolume = useLibraryMutation("volumes.track");
  const indexVolume = useLibraryMutation("volumes.index");
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
      path: props.initialPath || "",
      name: initialFolderName,
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
      jobOptions.filter((j) => j.presets.includes("Deep")).map((j) => j.id)
    )
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

  const handleVolumeSelect = async (volume: any) => {
    try {
      // Step 1: Track the volume
      const trackResult = await trackVolume.mutateAsync({
        fingerprint: volume.fingerprint,
        display_name: volume.display_name || volume.name,
      });

      // Step 2: Create a location for the volume's mount point
      const locationInput: LocationAddInput = {
        path: {
          Physical: {
            device_slug: "local",
            path: volume.mount_point || "/",
          },
        },
        name: volume.display_name || volume.name,
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
    // Validate path first
    const validateInput: ValidateLocationPathInput = {
      path: {
        Physical: {
          device_slug: "local",
          path: data.path,
        },
      },
    };

    let validation;
    try {
      validation = await client.query("locations.validate_path", validateInput);
    } catch (error) {
      console.error("Failed to validate path:", error);
      // Continue anyway if validation fails
    }

    // Show warning dialog if path is risky
    if (
      validation &&
      (validation.risk_level === "medium" || validation.risk_level === "high")
    ) {
      setValidationResult({
        riskLevel: validation.risk_level,
        warnings: validation.warnings,
        suggestion: validation.suggested_alternative || null,
      });
      setShowWarning(true);
      return; // Wait for user decision
    }

    // Path is safe or user bypassed warning - proceed with adding location
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

  // Handle warning dialog actions
  const handleProceedAnyway = async () => {
    setShowWarning(false);
    const data = localForm.getValues();

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
  };

  const handleUseVolumeIndexing = async () => {
    if (!validationResult?.suggestion) return;

    setShowWarning(false);

    try {
      const result = await indexVolume.mutateAsync({
        fingerprint: validationResult.suggestion.volume_fingerprint,
        scope: "Recursive",
      });

      dialog.state.open = false;

      console.log("Volume indexed:", result.message);
    } catch (error) {
      console.error("Failed to index volume:", error);
      localForm.setError("root", {
        type: "manual",
        message:
          error instanceof Error ? error.message : "Failed to index volume",
      });
    }
  };

  const handleCancelWarning = () => {
    setShowWarning(false);
    setValidationResult(null);
  };

  // Render category selection
  if (step === "category") {
    return (
      <StorageDialog
        description="Choose the type of storage you want to connect"
        dialog={dialog}
        form={dummyForm}
        hideButtons={true}
        icon={<CloudArrowUp size={20} weight="fill" />}
        title="Add Storage"
      >
        <div className="grid grid-cols-2 gap-3">
          {categories.map((category) => (
            <button
              className={clsx(
                "flex flex-col items-center gap-3 rounded-lg border p-6",
                "transition-all hover:scale-[1.02]",
                "border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover"
              )}
              key={category.id}
              onClick={() => handleCategorySelect(category.id)}
              type="button"
            >
              <img alt="" className="size-12" src={category.icon} />
              <div className="text-center">
                <div className="font-medium text-ink text-sm">
                  {category.label}
                </div>
                <div className="mt-1 text-ink-faint text-xs">
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
        description="Choose your cloud storage service"
        dialog={dialog}
        form={dummyForm}
        hideButtons={true}
        icon={<CloudArrowUp size={20} weight="fill" />}
        onBack={handleBack}
        showBackButton={true}
        title="Select Cloud Provider"
      >
        <div className="grid max-h-[400px] grid-cols-3 gap-3 overflow-y-auto pr-1">
          {cloudProviders.map((provider) => (
            <button
              className={clsx(
                "flex flex-col items-center gap-2 rounded-lg border p-4",
                "transition-all hover:scale-[1.02]",
                "border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover"
              )}
              key={provider.id}
              onClick={() => handleProviderSelect(provider)}
              type="button"
            >
              <img alt="" className="size-10" src={provider.icon} />
              <div className="text-center font-medium text-ink text-xs">
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
        description="Choose your network file protocol"
        dialog={dialog}
        form={dummyForm}
        hideButtons={true}
        icon={<img alt="" className="size-5" src={ServerIcon} />}
        onBack={handleBack}
        showBackButton={true}
        title="Select Network Protocol"
      >
        <div className="space-y-3">
          <div className="rounded-lg border border-accent/20 bg-accent/10 p-4 text-ink text-sm">
            <strong>Coming Soon</strong>
            <p className="mt-1 text-ink-dull">
              Network protocol support (SMB, NFS, SFTP, WebDAV) is currently in
              development. Check back in a future update!
            </p>
          </div>
          <div className="pointer-events-none grid grid-cols-2 gap-3 opacity-50">
            {networkProtocols.map((protocol) => (
              <button
                className={clsx(
                  "flex items-center gap-3 rounded-lg border p-4",
                  "border-app-line bg-app-box"
                )}
                disabled
                key={protocol.id}
                type="button"
              >
                <img alt="" className="size-8" src={protocol.icon} />
                <div className="text-left">
                  <div className="font-medium text-ink text-sm">
                    {protocol.name}
                  </div>
                  <div className="text-ink-faint text-xs">
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
        description="Select a connected drive to track"
        dialog={dialog}
        form={dummyForm}
        hideButtons={true}
        icon={<HardDrive size={20} weight="fill" />}
        onBack={handleBack}
        showBackButton={true}
        title="Track External Drive"
      >
        <div className="space-y-3">
          {volumes && volumes.length > 0 ? (
            <div className="max-h-[400px] space-y-2 overflow-y-auto pr-1">
              {volumes.map((volume) => (
                <button
                  className={clsx(
                    "flex w-full items-center gap-3 rounded-lg border p-3 text-left",
                    "transition-all hover:scale-[1.01]",
                    "border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover"
                  )}
                  key={volume.fingerprint}
                  onClick={() => handleVolumeSelect(volume)}
                  type="button"
                >
                  <img alt="" className="size-8" src={HDDIcon} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-medium text-ink text-sm">
                      {volume.display_name || volume.name}
                    </div>
                    <div className="text-ink-faint text-xs">
                      {volume.mount_point} • {volume.filesystem}
                    </div>
                  </div>
                  <div className="text-ink-dull text-xs">
                    {volume.total_capacity
                      ? (volume.total_capacity / 1e9).toFixed(0)
                      : "?"}{" "}
                    GB
                  </div>
                </button>
              ))}
            </div>
          ) : (
            <div className="rounded-lg border border-app-line bg-app-box p-6 text-center">
              <p className="text-ink-dull text-sm">
                No untracked external drives found. Connect a drive and refresh
                to see it here.
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
        description="Choose a folder to index and manage"
        dialog={dialog}
        form={dummyForm}
        hideButtons={true}
        icon={<Folder size={20} weight="fill" />}
        onBack={handleBack}
        showBackButton={true}
        title="Add Local Folder"
      >
        <div className="flex flex-col space-y-4">
          <div className="space-y-2">
            <Label>Browse</Label>
            <div className="relative">
              <Input
                className="pr-14"
                onChange={(e) => localForm.setValue("path", e.target.value)}
                placeholder="Select a custom folder"
                size="lg"
                value={localForm.watch("path") || ""}
              />
              <TopBarButton
                className="absolute top-1/2 right-2 -translate-y-1/2"
                icon={FolderOpen}
                onClick={handleBrowse}
              />
            </div>
          </div>

          {suggestedLocations && suggestedLocations.locations.length > 0 && (
            <div className="space-y-2">
              <Label>Suggested Locations</Label>
              <div className="grid max-h-[280px] grid-cols-2 gap-2 overflow-y-auto pr-1">
                {suggestedLocations.locations.map((loc) => (
                  <button
                    className="flex h-fit items-center gap-3 rounded-lg border border-app-line bg-app-box p-3 text-left transition-all hover:border-accent/50 hover:bg-app-hover"
                    key={loc.path}
                    onClick={() => handleSelectSuggested(loc.path, loc.name)}
                    type="button"
                  >
                    <Folder
                      className="size-5 shrink-0 text-accent"
                      weight="fill"
                    />
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-medium text-ink text-sm">
                        {loc.name}
                      </div>
                      <div className="truncate text-ink-faint text-xs">
                        {loc.path}
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
        ctaLabel="Add Location"
        description={localForm.watch("path")}
        dialog={dialog}
        form={localForm}
        icon={<Folder size={20} weight="fill" />}
        loading={addLocation.isPending}
        onBack={handleBack}
        onSubmit={onSubmitLocal}
        showBackButton={true}
        title="Configure Location"
      >
        <div className="space-y-4">
          <div className="space-y-2">
            <Label slug="name">Display Name</Label>
            <Input
              {...localForm.register("name")}
              className="bg-app-input"
              placeholder="My Documents"
              size="md"
            />
          </div>

          <Tabs.Root
            onValueChange={(v) => setTab(v as SettingsTab)}
            value={tab}
          >
            <Tabs.List>
              <Tabs.Trigger value="preset">Preset</Tabs.Trigger>
              <Tabs.Trigger value="jobs">
                Jobs {selectedJobs.size > 0 && `(${selectedJobs.size})`}
              </Tabs.Trigger>
            </Tabs.List>

            <Tabs.Content className="pt-3" value="preset">
              <div className="max-h-[280px] space-y-2 overflow-y-auto pr-1">
                <Label>Indexing Mode</Label>
                <div className="grid grid-cols-3 gap-2">
                  {indexModes.map((mode) => {
                    const isSelected = currentMode === mode.value;
                    return (
                      <button
                        className={clsx(
                          "rounded-lg border p-3 text-left transition-all",
                          isSelected
                            ? "border-accent bg-accent/5 ring-1 ring-accent"
                            : "border-app-line bg-app-box hover:bg-app-hover"
                        )}
                        key={mode.value}
                        onClick={() => handleModeChange(mode.value)}
                        type="button"
                      >
                        <div className="font-medium text-ink text-xs">
                          {mode.label}
                        </div>
                        <div className="mt-1 text-[11px] text-ink-faint leading-tight">
                          {mode.description}
                        </div>
                      </button>
                    );
                  })}
                </div>
              </div>
            </Tabs.Content>

            <Tabs.Content className="pt-3" value="jobs">
              <div className="max-h-[280px] space-y-3 overflow-y-auto pr-1">
                <p className="text-ink-faint text-xs">
                  Select which jobs to run after indexing. Extensions can add
                  more jobs.
                </p>
                <div className="grid grid-cols-2 gap-2">
                  {jobOptions.map((job) => {
                    const isSelected = selectedJobs.has(job.id);
                    return (
                      <button
                        className={clsx(
                          "flex items-start gap-2 rounded-lg border p-3 text-left transition-all",
                          isSelected
                            ? "border-accent bg-accent/5 ring-1 ring-accent"
                            : "border-app-line bg-app-box hover:bg-app-hover"
                        )}
                        key={job.id}
                        onClick={() => toggleJob(job.id)}
                        type="button"
                      >
                        <div className="min-w-0 flex-1">
                          <div className="font-medium text-ink text-xs">
                            {job.label}
                          </div>
                          <div className="mt-1 text-[11px] text-ink-faint leading-tight">
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
            <p className="text-red-500 text-xs">
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
        ctaLabel="Add Storage"
        description="Configure your cloud storage connection"
        dialog={dialog}
        form={cloudForm}
        icon={<img alt="" className="size-5" src={provider.icon} />}
        loading={addCloudVolume.isPending}
        onBack={handleBack}
        onSubmit={onSubmitCloud}
        showBackButton={true}
        title={`Add ${provider.name}`}
      >
        <div className="h-full space-y-4 overflow-y-auto pr-1">
          <div className="space-y-2">
            <Label>Display Name</Label>
            <Input
              {...cloudForm.register("display_name")}
              className="bg-app-input"
              placeholder={`My ${provider.name}`}
              size="md"
            />
          </div>

          {isS3Type && (
            <>
              <div className="space-y-2">
                <Label>Bucket</Label>
                <Input
                  {...cloudForm.register("bucket")}
                  className="bg-app-input"
                  placeholder="my-bucket"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Region</Label>
                <Input
                  {...cloudForm.register("region")}
                  className="bg-app-input"
                  placeholder="us-west-2"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Access Key ID</Label>
                <Input
                  {...cloudForm.register("access_key_id")}
                  className="bg-app-input"
                  placeholder="AKIA..."
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Secret Access Key</Label>
                <Input
                  {...cloudForm.register("secret_access_key")}
                  className="bg-app-input"
                  placeholder="••••••••••••••••••"
                  size="md"
                  type="password"
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
                    className="bg-app-input"
                    placeholder={
                      provider.id === "r2"
                        ? "https://account.r2.cloudflarestorage.com"
                        : provider.id === "minio"
                          ? "http://localhost:9000"
                          : "https://..."
                    }
                    size="md"
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
                  className="bg-app-input"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Client Secret</Label>
                <Input
                  {...cloudForm.register("client_secret")}
                  className="bg-app-input"
                  size="md"
                  type="password"
                />
              </div>
              <div className="space-y-2">
                <Label>Access Token</Label>
                <Input
                  {...cloudForm.register("access_token")}
                  className="bg-app-input"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Refresh Token</Label>
                <Input
                  {...cloudForm.register("refresh_token")}
                  className="bg-app-input"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Root Path (Optional)</Label>
                <Input
                  {...cloudForm.register("root")}
                  className="bg-app-input"
                  placeholder="/"
                  size="md"
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
                  className="bg-app-input"
                  placeholder="my-container"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Account Name</Label>
                <Input
                  {...cloudForm.register("account_name")}
                  className="bg-app-input"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Account Key</Label>
                <Input
                  {...cloudForm.register("account_key")}
                  className="bg-app-input"
                  size="md"
                  type="password"
                />
              </div>
              <div className="space-y-2">
                <Label>Endpoint (Optional)</Label>
                <Input
                  {...cloudForm.register("endpoint")}
                  className="bg-app-input"
                  placeholder="https://..."
                  size="md"
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
                  className="bg-app-input"
                  placeholder="my-gcs-bucket"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Service Account Credential (JSON)</Label>
                <textarea
                  {...cloudForm.register("credential")}
                  className="w-full rounded-lg border border-app-line bg-app-input px-3 py-2 font-mono text-ink text-sm"
                  placeholder='{"type": "service_account", ...}'
                  rows={6}
                />
              </div>
              <div className="space-y-2">
                <Label>Root Path (Optional)</Label>
                <Input
                  {...cloudForm.register("root")}
                  className="bg-app-input"
                  placeholder="/"
                  size="md"
                />
              </div>
              <div className="space-y-2">
                <Label>Endpoint (Optional)</Label>
                <Input
                  {...cloudForm.register("endpoint")}
                  className="bg-app-input"
                  placeholder="https://storage.googleapis.com"
                  size="md"
                />
              </div>
            </>
          )}

          {cloudForm.formState.errors.root && (
            <p className="text-red-500 text-xs">
              {cloudForm.formState.errors.root.message}
            </p>
          )}
        </div>
      </StorageDialog>
    );
  }

  // Render warning dialog for risky paths
  if (showWarning && validationResult) {
    return (
      <Dialog.Root onOpenChange={setShowWarning} open={showWarning}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <Dialog.Content className="fixed top-1/2 left-1/2 w-[480px] -translate-x-1/2 -translate-y-1/2 rounded-lg border border-app-line bg-app-box p-6 shadow-2xl">
            <Dialog.Title className="mb-4 font-semibold text-ink text-lg">
              {validationResult.riskLevel === "high"
                ? "⚠️ High Risk Path Detected"
                : "⚠️ Warning"}
            </Dialog.Title>

            <div className="mb-6 space-y-4">
              {validationResult.warnings.map((warning, i) => (
                <div className="space-y-2" key={i}>
                  <p className="text-ink text-sm">{warning.message}</p>
                  {warning.suggestion && (
                    <p className="text-ink-dull text-xs italic">
                      💡 {warning.suggestion}
                    </p>
                  )}
                </div>
              ))}

              {validationResult.suggestion && (
                <div className="space-y-3 rounded-lg border border-accent bg-accent/10 p-4">
                  <p className="font-medium text-ink text-sm">
                    Alternative Suggestion
                  </p>
                  <p className="text-ink-dull text-sm">
                    {validationResult.suggestion.message}
                  </p>
                  <Button
                    className="w-full"
                    onClick={handleUseVolumeIndexing}
                    size="sm"
                    variant="accent"
                  >
                    Index Volume: {validationResult.suggestion.volume_name}
                  </Button>
                </div>
              )}
            </div>

            <div className="flex justify-end gap-2">
              <Button onClick={handleCancelWarning} size="sm" variant="gray">
                Cancel
              </Button>
              <Button onClick={handleProceedAnyway} size="sm" variant="outline">
                Proceed Anyway
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    );
  }

  return null;
}
