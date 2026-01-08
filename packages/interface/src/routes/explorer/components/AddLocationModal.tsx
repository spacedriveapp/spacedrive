import { Folder, FolderOpen } from "@phosphor-icons/react";
import { NewLocation } from "@sd/assets/icons";
import type { IndexMode, LocationAddInput } from "@sd/ts-client";
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
import { useState } from "react";
import { useForm } from "react-hook-form";
import { usePlatform } from "../../../contexts/PlatformContext";
import {
  useLibraryMutation,
  useLibraryQuery,
} from "../../../contexts/SpacedriveContext";

interface AddLocationFormData {
  path: string;
  name: string;
  mode: IndexMode;
}

type ModalStep = "picker" | "settings";
type SettingsTab = "preset" | "jobs";

interface JobOption {
  id: string;
  label: string;
  description: string;
  presets: IndexMode[]; // Which presets include this job by default
  order: number; // Execution order
}

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
    presets: [], // Disabled by default
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

export function useAddLocationDialog(
  onLocationAdded?: (locationId: string) => void
) {
  return dialogManager.create((props) => (
    <AddLocationDialog {...props} onLocationAdded={onLocationAdded} />
  ));
}

function AddLocationDialog(props: {
  id: number;
  onLocationAdded?: (locationId: string) => void;
}) {
  const dialog = useDialog(props);
  const platform = usePlatform();
  const [step, setStep] = useState<ModalStep>("picker");
  const [tab, setTab] = useState<SettingsTab>("preset");

  const addLocation = useLibraryMutation("locations.add");
  const { data: suggestedLocations } = useLibraryQuery({
    type: "locations.suggested",
    input: null,
  });

  const form = useForm<AddLocationFormData>({
    defaultValues: {
      path: "",
      name: "",
      mode: "Deep",
    },
  });

  // Update selected jobs when preset mode changes
  const currentMode = form.watch("mode");
  const [selectedJobs, setSelectedJobs] = useState<Set<string>>(
    new Set(
      jobOptions.filter((j) => j.presets.includes("Deep")).map((j) => j.id)
    )
  );

  // Sync selected jobs with preset when mode changes
  const handleModeChange = (mode: IndexMode) => {
    form.setValue("mode", mode);
    // Update selected jobs based on preset
    const presetJobs = jobOptions.filter((j) => j.presets.includes(mode));
    setSelectedJobs(new Set(presetJobs.map((j) => j.id)));
  };

  const handleSelectSuggested = (path: string, name: string) => {
    form.setValue("path", path);
    form.setValue("name", name);
    setStep("settings");
  };

  const handleCancel = () => {
    form.setValue("path", "");
    form.setValue("name", "");
    setStep("picker");
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
      form.setValue("path", selected);
      // Auto-populate name with folder name
      const folderName = selected.split("/").pop() || "";
      form.setValue("name", folderName);
      // Move to settings step
      setStep("settings");
    }
  };

  const handlePickerKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && form.watch("path")) {
      e.preventDefault();
      const path = form.watch("path");
      const folderName = path.split("/").pop() || "";
      form.setValue("name", folderName);
      setStep("settings");
    }
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

  const onSubmit = form.handleSubmit(async (data) => {
    // Build job policies from selected jobs
    const job_policies: any = {};

    selectedJobs.forEach((jobId) => {
      job_policies[jobId] = { enabled: true };
    });

    const input: LocationAddInput = {
      path: {
        Physical: {
          device_slug: "local", // Backend determines actual device from context
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

      // Call the callback to navigate to the new location
      if (result?.id && props.onLocationAdded) {
        props.onLocationAdded(result.id);
      }
    } catch (error) {
      console.error("Failed to add location:", error);
      form.setError("root", {
        type: "manual",
        message:
          error instanceof Error ? error.message : "Failed to add location",
      });
    }
  });

  if (step === "picker") {
    return (
      <Dialog
        className="w-[520px]"
        description="Choose a folder to index and manage"
        dialog={dialog}
        form={form}
        icon={<img alt="" className="size-5" src={NewLocation} />}
        onCancelled={true}
        title="Add Location"
      >
        {/* Content */}
        <div className="flex flex-col space-y-4">
          <div className="space-y-2">
            <Label>Browse</Label>
            <div className="relative">
              <Input
                className="pr-14"
                onChange={(e) => form.setValue("path", e.target.value)}
                onKeyDown={handlePickerKeyDown}
                placeholder="Select a custom folder"
                size="lg"
                value={form.watch("path") || ""}
              />
              <TopBarButton
                className="absolute top-1/2 right-2 -translate-y-1/2"
                icon={FolderOpen}
                onClick={handleBrowse}
              />
            </div>
          </div>

          {/* Suggested Locations */}
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
      </Dialog>
    );
  }

  return (
    <Dialog
      buttonsSideContent={
        <Button onClick={handleCancel} size="sm" variant="gray">
          Back
        </Button>
      }
      className="w-[520px]"
      ctaLabel="Add Location"
      description={form.watch("path")}
      dialog={dialog}
      form={form}
      icon={<img alt="" className="size-5" src={NewLocation} />}
      loading={addLocation.isPending}
      onCancelled={true}
      onSubmit={onSubmit}
      title="Add Location"
    >
      <div className="space-y-4">
        {/* Name Input */}
        <div className="space-y-2">
          <Label slug="name">Display Name</Label>
          <Input
            {...form.register("name")}
            className="bg-app-input"
            placeholder="My Documents"
            size="md"
          />
        </div>

        {/* Tabs */}
        <Tabs.Root onValueChange={(v) => setTab(v as SettingsTab)} value={tab}>
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
                      className={`rounded-lg border p-3 text-left transition-all ${
                        isSelected
                          ? "border-accent bg-accent/5 ring-1 ring-accent"
                          : "border-app-line bg-app-box hover:bg-app-hover"
                      }
                        `}
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
                Select which jobs to run after indexing. Extensions can add more
                jobs.
              </p>
              <div className="grid grid-cols-2 gap-2">
                {jobOptions.map((job) => {
                  const isSelected = selectedJobs.has(job.id);
                  return (
                    <button
                      className={`flex items-start gap-2 rounded-lg border p-3 text-left transition-all ${
                        isSelected
                          ? "border-accent bg-accent/5 ring-1 ring-accent"
                          : "border-app-line bg-app-box hover:bg-app-hover"
                      }
                        `}
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

        {/* Error Display */}
        {form.formState.errors.root && (
          <p className="text-red-500 text-xs">
            {form.formState.errors.root.message}
          </p>
        )}
      </div>
    </Dialog>
  );
}
