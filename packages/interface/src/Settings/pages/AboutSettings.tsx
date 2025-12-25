import { useCoreQuery } from "../../context";

export function AboutSettings() {
  const { data: status } = useCoreQuery({ type: "core.status", input: {} });

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">About</h2>
        <p className="text-sm text-ink-dull">
          Information about Spacedrive.
        </p>
      </div>

      {/* Branding */}
      <div className="p-6 bg-app-box rounded-lg border border-app-line text-center">
        <div className="flex justify-center mb-4">
          <div className="w-16 h-16 bg-accent rounded-xl flex items-center justify-center">
            <svg
              className="w-10 h-10 text-white"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
            </svg>
          </div>
        </div>
        <h3 className="text-xl font-bold text-ink mb-1">Spacedrive</h3>
        <p className="text-sm text-ink-dull">
          A file explorer from the future.
        </p>
      </div>

      {/* Version Info */}
      <div className="p-4 bg-app-box rounded-lg border border-app-line space-y-3">
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
        <div className="flex justify-between items-center">
          <span className="text-sm text-ink">Data Directory</span>
          <span className="text-sm text-ink-dull font-mono truncate max-w-[200px]">
            {status?.system?.data_directory || "Loading..."}
          </span>
        </div>
        {status?.system?.instance_name && (
          <div className="flex justify-between items-center">
            <span className="text-sm text-ink">Instance</span>
            <span className="text-sm text-ink-dull">
              {status.system.instance_name}
            </span>
          </div>
        )}
      </div>

      {/* License */}
      <div className="p-4 bg-app-box rounded-lg border border-app-line">
        <div className="flex justify-between items-center">
          <span className="text-sm text-ink">License</span>
          <a
            href="https://github.com/spacedriveapp/spacedrive/blob/main/LICENSE"
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-accent hover:underline"
          >
            AGPL-3.0
          </a>
        </div>
      </div>

      {/* Links */}
      <div className="flex flex-wrap gap-3">
        <a
          href="https://spacedrive.com"
          target="_blank"
          rel="noopener noreferrer"
          className="flex-1 min-w-[120px] px-4 py-3 bg-app-box rounded-lg border border-app-line text-center hover:bg-app-hover transition-colors"
        >
          <span className="text-sm font-medium text-ink">Website</span>
        </a>
        <a
          href="https://github.com/spacedriveapp/spacedrive"
          target="_blank"
          rel="noopener noreferrer"
          className="flex-1 min-w-[120px] px-4 py-3 bg-app-box rounded-lg border border-app-line text-center hover:bg-app-hover transition-colors"
        >
          <span className="text-sm font-medium text-ink">GitHub</span>
        </a>
        <a
          href="https://discord.gg/spacedrive"
          target="_blank"
          rel="noopener noreferrer"
          className="flex-1 min-w-[120px] px-4 py-3 bg-app-box rounded-lg border border-app-line text-center hover:bg-app-hover transition-colors"
        >
          <span className="text-sm font-medium text-ink">Discord</span>
        </a>
      </div>

      {/* Changelog */}
      <div className="p-4 bg-app-box rounded-lg border border-app-line">
        <h3 className="text-sm font-medium text-ink mb-2">Changelog</h3>
        <p className="text-xs text-ink-dull mb-2">
          See what's new in the latest release.
        </p>
        <a
          href="https://github.com/spacedriveapp/spacedrive/releases"
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm text-accent hover:underline"
        >
          View Release Notes
        </a>
      </div>
    </div>
  );
}
