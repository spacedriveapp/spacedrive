import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../contexts/SpacedriveContext";

interface DeviceSettingsForm {
  name: string;
  slug: string;
}

export function GeneralSettings() {
  const statusQuery = useCoreQuery({ type: "core.status", input: null as any });
  const configQuery = useCoreQuery({ type: "config.app.get", input: null as any });
  const updateDevice = useCoreMutation("device.update");
  const resetData = useCoreMutation("core.reset");

  const { data: status } = statusQuery;
  const { data: config } = configQuery;

  const deviceForm = useForm<DeviceSettingsForm>({
    values: {
      name: status?.device_info?.name || "",
      slug: status?.device_info?.slug || "",
    },
  });

  const onDeviceSubmit = deviceForm.handleSubmit(async (data) => {
    await updateDevice.mutateAsync({
      name: data.name,
      slug: data.slug,
    });
    statusQuery.refetch();
  });

  const handleResetData = () => {
    const confirmed = window.confirm(
      "Reset All Data\n\nThis will permanently delete all libraries, settings, and cached data. The app will need to be restarted. Are you sure?"
    );

    if (confirmed) {
      resetData.mutate(
        { confirm: true },
        {
          onSuccess: (result) => {
            alert(
              result.message || "Data has been reset. Please restart the application."
            );
          },
          onError: (error) => {
            alert("Error: " + (error.message || "Failed to reset data"));
          },
        }
      );
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">General</h2>
        <p className="text-sm text-ink-dull">
          Configure general application settings.
        </p>
      </div>

      <div className="space-y-4">
        {/* Device Configuration */}
        <form onSubmit={onDeviceSubmit} className="p-4 bg-app-box rounded-lg border border-app-line space-y-4">
          <h3 className="text-sm font-medium text-ink">Device</h3>

          <label className="block">
            <span className="text-sm font-medium text-ink mb-1 block">Device Name</span>
            <p className="text-xs text-ink-dull mb-2">
              User-friendly name for this device
            </p>
            <input
              type="text"
              {...deviceForm.register("name")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent"
              placeholder="My Computer"
            />
          </label>

          <label className="block">
            <span className="text-sm font-medium text-ink mb-1 block">Device Slug</span>
            <p className="text-xs text-ink-dull mb-2">
              Unique identifier for this device (alphanumeric and hyphens only)
            </p>
            <input
              type="text"
              {...deviceForm.register("slug")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent font-mono"
              placeholder="my-computer"
            />
          </label>

          {deviceForm.formState.isDirty && (
            <button
              type="submit"
              disabled={updateDevice.isPending}
              className="px-4 py-2 bg-accent hover:bg-accent-deep text-white rounded-md text-sm font-medium transition-colors disabled:opacity-50"
            >
              {updateDevice.isPending ? "Saving..." : "Save Changes"}
            </button>
          )}
        </form>

        {/* Version Info */}
        <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-3">
          <h3 className="text-sm font-medium text-ink">Version Information</h3>
          <div className="flex justify-between items-center">
            <span className="text-sm text-ink">Version</span>
            <span className="text-sm text-ink-dull font-mono">
              {status?.version || "Loading..."}
            </span>
          </div>
          <div className="flex justify-between items-center">
            <span className="text-sm text-ink">Built</span>
            <span className="text-sm text-ink-dull font-mono">
              {status?.built_at || "Loading..."}
            </span>
          </div>
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Data Directory</h3>
          <p className="text-xs text-ink-dull mb-2">Where Spacedrive stores its data</p>
          <code className="block text-xs text-ink-dull bg-app rounded px-2 py-1 overflow-x-auto">
            {config?.data_dir || status?.system?.data_directory || "Loading..."}
          </code>
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-medium text-ink mb-1">Reset All Data</h3>
              <p className="text-xs text-ink-dull">
                Permanently delete all libraries and settings
              </p>
            </div>
            <button
              type="button"
              onClick={handleResetData}
              disabled={resetData.isPending}
              className="px-4 py-2 bg-red-600 hover:bg-red-700 disabled:opacity-50 text-white rounded-lg text-sm font-medium transition-colors"
            >
              {resetData.isPending ? "Resetting..." : "Reset"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
