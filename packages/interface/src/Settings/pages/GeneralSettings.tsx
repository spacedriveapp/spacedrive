import { useForm } from "react-hook-form";
import { useCoreQuery, useCoreMutation } from "../../context";

interface GeneralSettingsForm {
  log_level: string;
}

export function GeneralSettings() {
  const { data: status } = useCoreQuery({ type: "core.status", input: {} });
  const { data: config, refetch } = useCoreQuery({ type: "config.app.get", input: {} });
  const updateConfig = useCoreMutation("config.app.update");
  const resetData = useCoreMutation("core.reset");

  const form = useForm<GeneralSettingsForm>({
    values: {
      log_level: config?.log_level || "info",
    },
  });

  const onSubmit = form.handleSubmit(async (data) => {
    await updateConfig.mutateAsync({
      log_level: data.log_level,
    });
    refetch();
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

      <form onSubmit={onSubmit} className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <label className="block">
            <span className="text-sm font-medium text-ink mb-1 block">Log Level</span>
            <p className="text-xs text-ink-dull mb-2">Set the verbosity of daemon logs</p>
            <select
              {...form.register("log_level")}
              className="w-full px-3 py-2 bg-app border border-app-line rounded-md text-ink text-sm focus:outline-none focus:ring-2 focus:ring-accent"
            >
              <option value="trace">Trace</option>
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warn</option>
              <option value="error">Error</option>
            </select>
          </label>
          {form.formState.isDirty && (
            <button
              type="submit"
              disabled={updateConfig.isPending}
              className="mt-3 px-4 py-2 bg-accent hover:bg-accent-deep text-white rounded-md text-sm font-medium transition-colors disabled:opacity-50"
            >
              {updateConfig.isPending ? "Saving..." : "Save"}
            </button>
          )}
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Data Directory</h3>
          <p className="text-xs text-ink-dull mb-2">Where Spacedrive stores its data</p>
          <code className="block text-xs text-ink-dull bg-app rounded px-2 py-1 overflow-x-auto">
            {config?.data_dir || status?.system?.data_directory || "Loading..."}
          </code>
        </div>

        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Instance Name</h3>
          <p className="text-xs text-ink-dull mb-2">Name of this Spacedrive instance</p>
          <span className="text-sm text-ink">
            {status?.system?.instance_name || status?.device_info?.name || "Default Instance"}
          </span>
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
      </form>
    </div>
  );
}
