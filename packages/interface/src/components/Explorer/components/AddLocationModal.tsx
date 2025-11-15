import { useState } from "react";
import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";
import { Folder, FolderOpen } from "@phosphor-icons/react";
import {
  Button,
  Input,
  Label,
  Dialog,
  dialogManager,
  useDialog,
  TopBarButton,
} from "@sd/ui";
import * as Tabs from "@sd/ui/Tabs";
import type {
  IndexMode,
  LocationAddInput,
} from "@sd/ts-client/generated/types";
import { useLibraryMutation, useLibraryQuery } from "../../../context";
import { usePlatform } from "../../../platform";
import { NewLocation } from "@sd/assets/icons";

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
    id: "extract_metadata",
    label: "Extract Metadata",
    description: "Extract EXIF, ID3, and other metadata from files",
    presets: ["Content", "Deep"],
    order: 1,
  },
  {
    id: "generate_thumbnails",
    label: "Generate Thumbnails",
    description: "Create image/video thumbnails for preview",
    presets: ["Deep"],
    order: 2,
  },
  {
    id: "media_analysis",
    label: "Media Analysis",
    description: "Analyze media dimensions and properties",
    presets: ["Content", "Deep"],
    order: 3,
  },
  {
    id: "detect_duplicates",
    label: "Detect Duplicates",
    description: "Find duplicate files via content hashing",
    presets: [],
    order: 4,
  },
  {
    id: "extract_text",
    label: "Extract Text",
    description: "OCR and text extraction from images/PDFs",
    presets: [],
    order: 5,
  },
  {
    id: "archive_extraction",
    label: "Archive Extraction",
    description: "Index contents of ZIP, TAR files",
    presets: [],
    order: 6,
  },
  {
    id: "checksum_generation",
    label: "Checksum Generation",
    description: "Generate file integrity checksums",
    presets: [],
    order: 7,
  },
  {
    id: "ai_tagging",
    label: "AI Tagging",
    description: "Auto-tag files using AI analysis",
    presets: [],
    order: 8,
  },
];

export function useAddLocationDialog(
  onLocationAdded?: (locationId: string) => void,
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
      jobOptions.filter((j) => j.presets.includes("Deep")).map((j) => j.id),
    ),
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
    const input: LocationAddInput = {
      path: {
        Physical: {
          device_slug: "local", // Backend determines actual device from context
          path: data.path,
        },
      },
      name: data.name || null,
      mode: data.mode,
    };

    try {
      const result = await addLocation.mutateAsync(input);
      // TODO: Dispatch additional jobs based on selectedJobs
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
        dialog={dialog}
        form={form}
        title="Add Location"
        icon={<img src={NewLocation} alt="" className="size-5" />}
        description="Choose a folder to index and manage"
        className="w-[520px]"
        onCancelled={true}
      >
        {/* Content */}
        <div className="space-y-4 flex flex-col">
          <div className="space-y-2">
            <Label>Browse</Label>
            <div className="relative">
              <Input
                value={form.watch("path") || ""}
                onChange={(e) => form.setValue("path", e.target.value)}
                onKeyDown={handlePickerKeyDown}
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

          {/* Suggested Locations */}
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

  return (
    <Dialog
      dialog={dialog}
      form={form}
      onSubmit={onSubmit}
      title="Add Location"
      icon={<img src={NewLocation} alt="" className="size-5" />}
      description={form.watch("path")}
      ctaLabel="Add Location"
      onCancelled={true}
      loading={addLocation.isPending}
      className="w-[520px]"
      buttonsSideContent={
        <Button variant="gray" size="sm" onClick={handleCancel}>
          Back
        </Button>
      }
    >
      <div className="space-y-4">
        {/* Name Input */}
        <div className="space-y-2">
          <Label slug="name">Display Name</Label>
          <Input
            {...form.register("name")}
            size="md"
            placeholder="My Documents"
            className="bg-app-input"
          />
        </div>

        {/* Tabs */}
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
                      className={`
                          rounded-lg border p-3 text-left transition-all
                          ${
                            isSelected
                              ? "border-accent bg-accent/5 ring-1 ring-accent"
                              : "border-app-line bg-app-box hover:bg-app-hover"
                          }
                        `}
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
                Select which jobs to run after indexing. Extensions can add more
                jobs.
              </p>
              <div className="grid grid-cols-2 gap-2">
                {jobOptions.map((job) => {
                  const isSelected = selectedJobs.has(job.id);
                  return (
                    <button
                      key={job.id}
                      type="button"
                      onClick={() => toggleJob(job.id)}
                      className={`
                          flex items-start gap-2 rounded-lg border p-3 text-left transition-all
                          ${
                            isSelected
                              ? "border-accent bg-accent/5 ring-1 ring-accent"
                              : "border-app-line bg-app-box hover:bg-app-hover"
                          }
                        `}
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

        {/* Error Display */}
        {form.formState.errors.root && (
          <p className="text-xs text-red-500">
            {form.formState.errors.root.message}
          </p>
        )}
      </div>
    </Dialog>
  );
}
