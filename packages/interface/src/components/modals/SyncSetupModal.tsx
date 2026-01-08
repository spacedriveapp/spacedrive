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
import { useState } from "react";
import { useForm } from "react-hook-form";
import {
  useCoreMutation,
  useCoreQuery,
  useSpacedriveClient,
} from "../../contexts/SpacedriveContext";

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
    null
  );
  const [selectedAction, setSelectedAction] = useState<"share" | "join" | null>(
    null
  );
  const [selectedRemoteLibrary, setSelectedRemoteLibrary] =
    useState<RemoteLibraryInfo | null>(null);

  // Get current device info and library
  const {
    data: coreStatus,
    isLoading,
    error,
    isFetching,
  } = useCoreQuery({
    type: "core.status",
    input: null as any, // Unit type () in Rust = null in JSON
  });

  console.log({
    coreStatus,
    isLoading,
    error: error?.message || error,
    isFetching,
    hasData: !!coreStatus,
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
    }
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
    library?: RemoteLibraryInfo
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
      !(selectedDevice && selectedAction && currentLibraryId && currentDeviceId)
    )
      return;

    setStep("executing");

    // Build the LibrarySyncAction
    let action: LibrarySyncAction;
    let remoteLibraryId: string | undefined;

    // Get current library info
    const currentLibrary = coreStatus?.libraries.find(
      (lib) => lib.id === currentLibraryId
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
            isLoading={discoveryQuery.isLoading}
            isOnline={discoveryQuery.data?.isOnline}
            onBack={() => setStep("select-device")}
            onSelectAction={handleActionSelect}
            remoteLibraries={discoveryQuery.data?.libraries || []}
          />
        );

      case "confirm":
        return (
          <ConfirmStep
            action={selectedAction!}
            device={selectedDevice!}
            onBack={() => setStep("choose-action")}
            onConfirm={handleConfirm}
            remoteLibrary={selectedRemoteLibrary}
          />
        );

      case "executing":
        return (
          <ExecutingStep
            error={syncSetupMutation.error?.message}
            isLoading={syncSetupMutation.isPending}
          />
        );
    }
  };

  return (
    <Dialog
      closeBtn
      description="Sync your library with another device"
      dialog={dialog}
      form={form}
      hideButtons
      icon={<ArrowsClockwise />}
      title="Setup Library Sync"
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
        <DeviceMobile className="text-ink-faint" size={48} />
        <div className="text-center">
          <p className="text-ink-dull">No paired devices found</p>
          <p className="text-ink-faint text-sm">
            Pair a device first using the "Pair Device" button
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-ink-dull text-sm">
        Select a paired device to sync your library with:
      </p>
      <div className="space-y-2">
        {devices.map((device) => (
          <button
            className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
            key={device.id}
            onClick={() => onSelect(device)}
          >
            <div className="flex items-start justify-between">
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <DeviceMobile size={20} />
                  <h3 className="font-medium text-ink">{device.name}</h3>
                  {device.isConnected && (
                    <span className="rounded-full bg-green-500 px-2 py-0.5 text-white text-xs">
                      Connected
                    </span>
                  )}
                </div>
                <p className="mt-1 text-ink-dull text-sm">
                  {device.deviceType} • {device.osVersion}
                </p>
                <p className="text-ink-faint text-xs">
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
    library?: RemoteLibraryInfo
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
        <DeviceMobile className="text-ink-faint" size={48} />
        <div className="text-center">
          <p className="text-ink-dull">Device is offline</p>
          <p className="text-ink-faint text-sm">
            {device.name} must be online to set up sync
          </p>
        </div>
        <Button onClick={onBack} variant="outline">
          Back
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div>
        <p className="text-ink-dull text-sm">
          Syncing with:{" "}
          <span className="font-medium text-ink">{device.name}</span>
        </p>
      </div>

      <div className="space-y-3">
        <h3 className="font-medium text-ink">Choose an action:</h3>

        {/* Share Local Library */}
        <button
          className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
          onClick={() => onSelectAction("share")}
        >
          <div className="flex items-start gap-3">
            <Share className="mt-1 text-accent" size={24} />
            <div className="flex-1">
              <h4 className="font-medium text-ink">
                Share my library to this device
              </h4>
              <p className="mt-1 text-ink-dull text-sm">
                Create a shared library from your local library. The other
                device will receive a copy.
              </p>
            </div>
          </div>
        </button>

        {/* Join Remote Library */}
        {remoteLibraries.length > 0 ? (
          <div className="space-y-2">
            <h4 className="font-medium text-ink text-sm">
              Or join a library from {device.name}:
            </h4>
            {remoteLibraries.map((library) => (
              <button
                className="w-full rounded-lg border border-app-line bg-app-box p-4 text-left transition-colors hover:border-accent hover:bg-app-darkBox"
                key={library.id}
                onClick={() => onSelectAction("join", library)}
              >
                <div className="flex items-start gap-3">
                  <SignIn className="mt-1 text-accent" size={24} />
                  <div className="flex-1">
                    <h4 className="font-medium text-ink">{library.name}</h4>
                    <p className="mt-1 text-ink-dull text-sm">
                      {library.statistics.total_files.toLocaleString()} files •{" "}
                      {library.statistics.location_count.toLocaleString()}{" "}
                      locations
                    </p>
                    <p className="text-ink-faint text-xs">
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
            <p className="text-ink-faint text-sm">
              No libraries found on {device.name}
            </p>
          </div>
        )}
      </div>

      <div className="flex justify-start">
        <Button onClick={onBack} variant="outline">
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
        <Button className="flex-1" onClick={onBack} variant="outline">
          Back
        </Button>
        <Button className="flex-1" onClick={onConfirm} variant="accent">
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
      <CheckCircle className="text-green-500" size={48} />
      <div className="text-center">
        <p className="font-medium text-ink">Sync setup complete!</p>
        <p className="text-ink-dull text-sm">Your library is now syncing</p>
      </div>
    </div>
  );
}
