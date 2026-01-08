import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import {
  ArrowsClockwise,
  CheckCircle,
  CircleNotch,
  DeviceMobile,
  Share,
  SignIn,
} from "@phosphor-icons/react";
import type {
  LibrarySyncAction,
  PairedDeviceInfo,
  RemoteLibraryInfo,
} from "@sd/ts-client";
import { Button, Dialog, dialogManager, useDialog } from "@sd/ui";
import { useCoreQuery, useCoreMutation, useSpacedriveClient } from "../../contexts/SpacedriveContext";

interface SyncSetupDialogProps {
  id: number;
}

type SyncStep = "select-device" | "choose-action" | "confirm" | "executing";

export function useSyncSetupDialog() {
  return dialogManager.create((props: SyncSetupDialogProps) => (
    <SyncSetupDialog {...props} />
  ));
}

function SyncSetupDialog(props: SyncSetupDialogProps) {
  const dialog = useDialog(props);
  const client = useSpacedriveClient();
  const [step, setStep] = useState<SyncStep>("select-device");
  const [selectedDevice, setSelectedDevice] = useState<PairedDeviceInfo | null>(
    null,
  );
  const [selectedAction, setSelectedAction] = useState<"share" | "join" | null>(
    null,
  );
  const [selectedRemoteLibrary, setSelectedRemoteLibrary] =
    useState<RemoteLibraryInfo | null>(null);

  // Get current device info and library
  const { data: coreStatus, isLoading, error, isFetching } = useCoreQuery({
    type: "core.status",
    input: null as any, // Unit type () in Rust = null in JSON
  });

  console.log({
    coreStatus,
    isLoading,
    error: error?.message || error,
    isFetching,
    hasData: !!coreStatus
  });

  const currentLibraryId = client.getCurrentLibraryId();
  const currentDeviceId = coreStatus?.device_info.id;

  // Query paired devices
  const pairedDevicesQuery = useCoreQuery({
    type: "network.devices.list",
    input: { connectedOnly: false },
  });

  // Query remote libraries when device is selected
  const discoveryQuery = useCoreQuery(
    {
      type: "network.sync_setup.discover",
      input: { deviceId: selectedDevice?.id || "" },
    },
    {
      enabled: selectedDevice !== null,
    },
  );

  // Sync setup mutation
  const syncSetupMutation = useCoreMutation("network.sync_setup", {
    onSuccess: () => {
      // Close dialog on success
      dialog.state.open = false;
    },
  });

  const form = useForm();

  const handleDeviceSelect = (device: PairedDeviceInfo) => {
    setSelectedDevice(device);
    setStep("choose-action");
  };

  const handleActionSelect = (
    action: "share" | "join",
    library?: RemoteLibraryInfo,
  ) => {
    setSelectedAction(action);
    if (action === "join" && library) {
      setSelectedRemoteLibrary(library);
    }
    setStep("confirm");
  };

  const handleConfirm = async () => {
    console.log("Confirming sync setup", {
      selectedDevice,
      selectedAction,
      currentLibraryId,
      currentDeviceId,
    });
    if (
      !selectedDevice ||
      !selectedAction ||
      !currentLibraryId ||
      !currentDeviceId
    )
      return;

    setStep("executing");

    // Build the LibrarySyncAction
    let action: LibrarySyncAction;
    let remoteLibraryId: string | undefined;

    // Get current library info
    const currentLibrary = coreStatus?.libraries.find(
      (lib) => lib.id === currentLibraryId,
    );
    const libraryName = currentLibrary?.name || "My Library";

    if (selectedAction === "share") {
      action = {
        type: "shareLocalLibrary",
        libraryName,
      };
    } else if (selectedAction === "join" && selectedRemoteLibrary) {
      action = {
        type: "joinRemoteLibrary",
        remoteLibraryId: selectedRemoteLibrary.id,
        remoteLibraryName: selectedRemoteLibrary.name,
      };
      remoteLibraryId = selectedRemoteLibrary.id;
    } else {
      return;
    }

    const data = {
      localDeviceId: currentDeviceId,
      remoteDeviceId: selectedDevice.id,
      localLibraryId: currentLibraryId,
      remoteLibraryId,
      action,
      leaderDeviceId: currentDeviceId, // Deprecated but required
    };

    console.log({ data });

    // Execute sync setup
    syncSetupMutation.mutate(data);
  };

  const renderContent = () => {
    switch (step) {
      case "select-device":
        return (
          <SelectDeviceStep
            devices={pairedDevicesQuery.data?.devices || []}
            isLoading={pairedDevicesQuery.isLoading}
            onSelect={handleDeviceSelect}
          />
        );

      case "choose-action":
        return (
          <ChooseActionStep
            device={selectedDevice!}
            remoteLibraries={discoveryQuery.data?.libraries || []}
            isOnline={discoveryQuery.data?.isOnline || false}
            isLoading={discoveryQuery.isLoading}
            onSelectAction={handleActionSelect}
            onBack={() => setStep("select-device")}
          />
        );

      case "confirm":
        return (
          <ConfirmStep
            device={selectedDevice!}
            action={selectedAction!}
            remoteLibrary={selectedRemoteLibrary}
            onBack={() => setStep("choose-action")}
            onConfirm={handleConfirm}
          />
        );

      case "executing":
        return (
          <ExecutingStep
            isLoading={syncSetupMutation.isPending}
            error={syncSetupMutation.error?.message}
          />
        );
    }
  };

  return (
    <Dialog
      dialog={dialog}
      form={form}
      title="Setup Library Sync"
      description="Sync your library with another device"
      icon={<ArrowsClockwise />}
      closeBtn
      hideButtons
    >
      <div className="min-h-[400px]">{renderContent()}</div>
    </Dialog>
  );
}

// Step 1: Select Device
interface SelectDeviceStepProps {
  devices: PairedDeviceInfo[];
  isLoading: boolean;
  onSelect: (device: PairedDeviceInfo) => void;
}

function SelectDeviceStep({
  devices,
  isLoading,
  onSelect,
}: SelectDeviceStepProps) {
  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <CircleNotch className="animate-spin" size={32} />
      </div>
    );
  }

  if (devices.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center space-y-4">
        <DeviceMobile size={48} className="text-ink-faint" />
        <div className="text-center">
          <p className="text-ink-dull">No paired devices found</p>
          <p className="text-sm text-ink-faint">
            Pair a device first using the "Pair Device" button
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-ink-dull">
        Select a paired device to sync your library with:
      </p>
      <div className="space-y-2">
        {devices.map((device) => (
          <button
            key={device.id}
            onClick={() => onSelect(device)}
            className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
          >
            <div className="flex items-start justify-between">
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <DeviceMobile size={20} />
                  <h3 className="font-medium text-ink">{device.name}</h3>
                  {device.isConnected && (
                    <span className="rounded-full bg-green-500 px-2 py-0.5 text-xs text-white">
                      Connected
                    </span>
                  )}
                </div>
                <p className="mt-1 text-sm text-ink-dull">
                  {device.deviceType} • {device.osVersion}
                </p>
                <p className="text-xs text-ink-faint">
                  Last seen: {new Date(device.lastSeen).toLocaleString()}
                </p>
              </div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

// Step 2: Choose Action
interface ChooseActionStepProps {
  device: PairedDeviceInfo;
  remoteLibraries: RemoteLibraryInfo[];
  isOnline: boolean;
  isLoading: boolean;
  onSelectAction: (
    action: "share" | "join",
    library?: RemoteLibraryInfo,
  ) => void;
  onBack: () => void;
}

function ChooseActionStep({
  device,
  remoteLibraries,
  isOnline,
  isLoading,
  onSelectAction,
  onBack,
}: ChooseActionStepProps) {
  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <CircleNotch className="animate-spin" size={32} />
        <p className="ml-3 text-ink-dull">Discovering libraries...</p>
      </div>
    );
  }

  if (!isOnline) {
    return (
      <div className="flex h-full flex-col items-center justify-center space-y-4">
        <DeviceMobile size={48} className="text-ink-faint" />
        <div className="text-center">
          <p className="text-ink-dull">Device is offline</p>
          <p className="text-sm text-ink-faint">
            {device.name} must be online to set up sync
          </p>
        </div>
        <Button variant="outline" onClick={onBack}>
          Back
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm text-ink-dull">
          Syncing with:{" "}
          <span className="font-medium text-ink">{device.name}</span>
        </p>
      </div>

      <div className="space-y-3">
        <h3 className="font-medium text-ink">Choose an action:</h3>

        {/* Share Local Library */}
        <button
          onClick={() => onSelectAction("share")}
          className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
        >
          <div className="flex items-start gap-3">
            <Share size={24} className="mt-1 text-accent" />
            <div className="flex-1">
              <h4 className="font-medium text-ink">
                Share my library to this device
              </h4>
              <p className="mt-1 text-sm text-ink-dull">
                Create a shared library from your local library. The other
                device will receive a copy.
              </p>
            </div>
          </div>
        </button>

        {/* Join Remote Library */}
        {remoteLibraries.length > 0 ? (
          <div className="space-y-2">
            <h4 className="text-sm font-medium text-ink">
              Or join a library from {device.name}:
            </h4>
            {remoteLibraries.map((library) => (
              <button
                key={library.id}
                onClick={() => onSelectAction("join", library)}
                className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
              >
                <div className="flex items-start gap-3">
                  <SignIn size={24} className="mt-1 text-accent" />
                  <div className="flex-1">
                    <h4 className="font-medium text-ink">{library.name}</h4>
                    <p className="mt-1 text-sm text-ink-dull">
                      {library.statistics.total_files.toLocaleString()} files •{" "}
                      {library.statistics.location_count.toLocaleString()}{" "}
                      locations
                    </p>
                    <p className="text-xs text-ink-faint">
                      Created:{" "}
                      {new Date(library.createdAt).toLocaleDateString()}
                    </p>
                  </div>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <div className="rounded-lg border border-app-line bg-app-box p-4">
            <p className="text-sm text-ink-faint">
              No libraries found on {device.name}
            </p>
          </div>
        )}
      </div>

      <div className="flex justify-start">
        <Button variant="outline" onClick={onBack}>
          Back
        </Button>
      </div>
    </div>
  );
}

// Step 3: Confirm
interface ConfirmStepProps {
  device: PairedDeviceInfo;
  action: "share" | "join";
  remoteLibrary: RemoteLibraryInfo | null;
  onBack: () => void;
  onConfirm: () => void;
}

function ConfirmStep({
  device,
  action,
  remoteLibrary,
  onBack,
  onConfirm,
}: ConfirmStepProps) {
  return (
    <div className="space-y-6">
      <div className="rounded-lg bg-app-darkBox p-4">
        <h3 className="mb-3 font-medium text-ink">Sync Configuration</h3>
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-ink-dull">Remote Device:</span>
            <span className="font-medium text-ink">{device.name}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-ink-dull">Action:</span>
            <span className="font-medium text-ink">
              {action === "share" ? "Share My Library" : "Join Remote Library"}
            </span>
          </div>
          {action === "join" && remoteLibrary && (
            <div className="flex justify-between">
              <span className="text-ink-dull">Remote Library:</span>
              <span className="font-medium text-ink">{remoteLibrary.name}</span>
            </div>
          )}
        </div>
      </div>

      <div className="rounded-lg border border-yellow-500/20 bg-yellow-500/10 p-4">
        <p className="text-sm text-yellow-200">
          {action === "share"
            ? "This will create a synchronized copy of your library on the remote device. Both devices will stay in sync."
            : "This will download the remote library to your device. Your local library will sync with theirs."}
        </p>
      </div>

      <div className="flex gap-2">
        <Button variant="outline" onClick={onBack} className="flex-1">
          Back
        </Button>
        <Button variant="accent" onClick={onConfirm} className="flex-1">
          Confirm & Setup Sync
        </Button>
      </div>
    </div>
  );
}

// Step 4: Executing
interface ExecutingStepProps {
  isLoading: boolean;
  error?: string;
}

function ExecutingStep({ isLoading, error }: ExecutingStepProps) {
  if (isLoading) {
    return (
      <div className="flex h-full flex-col items-center justify-center space-y-4">
        <CircleNotch className="animate-spin" size={48} />
        <p className="text-ink-dull">Setting up sync...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center space-y-4">
        <div className="text-red-500">
          <p className="font-medium">Sync setup failed</p>
          <p className="text-sm">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col items-center justify-center space-y-4">
      <CheckCircle size={48} className="text-green-500" />
      <div className="text-center">
        <p className="font-medium text-ink">Sync setup complete!</p>
        <p className="text-sm text-ink-dull">Your library is now syncing</p>
      </div>
    </div>
  );
}