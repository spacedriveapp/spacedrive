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
  CircleButton,
  TabsRoot,
  TabsList,
  TabsTrigger,
  TabsContent,
} from "@spacedrive/primitives";
import type {
  LocationAddInput,
} from "@sd/ts-client";
import { useLibraryMutation, useLibraryQuery } from "../../../contexts/SpacedriveContext";
import { usePlatform } from "../../../contexts/PlatformContext";
import { NewLocation } from "@sd/assets/icons";

interface AddLocationFormData {
  path: string;
  name: string;
}

type ModalStep = "picker" | "settings";

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

  const addLocation = useLibraryMutation("locations.add");
  const { data: suggestedLocations } = useLibraryQuery({
    type: "locations.suggested",
    input: null,
  });

  const form = useForm<AddLocationFormData>({
    defaultValues: {
      path: "",
      name: "",
    },
  });

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

  const onSubmit = form.handleSubmit(async (data) => {
    const input: LocationAddInput = {
      path: {
        Physical: {
          device_slug: "local", // Backend determines actual device from context
          path: data.path,
        },
      },
      name: data.name || null,
    };

    try {
      const result = await addLocation.mutateAsync(input);
      dialog.state.open = false;

      // Call the callback to navigate to the new location
      if (result?.location_id && props.onLocationAdded) {
        props.onLocationAdded(result.location_id);
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
              <CircleButton
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