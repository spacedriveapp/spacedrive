import clsx from "clsx";
import { useState } from "react";
import { useCoreMutation } from "../../contexts/SpacedriveContext";

interface SettingsSidebarProps {
  currentPage: string;
  onPageChange: (page: string) => void;
}

function SettingsSidebar({ currentPage, onPageChange }: SettingsSidebarProps) {
  const sections = [
    { id: "general", label: "General" },
    { id: "library", label: "Library" },
    { id: "privacy", label: "Privacy" },
    { id: "about", label: "About" },
  ];

  return (
    <div className="space-y-1">
      {sections.map((section) => (
        <button
          className={clsx(
            "w-full rounded-md px-3 py-2 text-left font-medium text-sm transition-colors",
            currentPage === section.id
              ? "bg-sidebar-selected text-sidebar-ink"
              : "text-sidebar-inkDull hover:bg-sidebar-box hover:text-sidebar-ink"
          )}
          key={section.id}
          onClick={() => onPageChange(section.id)}
        >
          {section.label}
        </button>
      ))}
    </div>
  );
}

interface SettingsContentProps {
  page: string;
}

function SettingsContent({ page }: SettingsContentProps) {
  switch (page) {
    case "general":
      return <GeneralSettings />;
    case "library":
      return <LibrarySettings />;
    case "privacy":
      return <PrivacySettings />;
    case "about":
      return <AboutSettings />;
    default:
      return <GeneralSettings />;
  }
}

function GeneralSettings() {
  const resetData = useCoreMutation("core.reset");

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
              result.message ||
                "Data has been reset. Please restart the application."
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
        <h2 className="mb-2 font-semibold text-ink text-lg">General</h2>
        <p className="text-ink-dull text-sm">
          Configure general application settings.
        </p>
      </div>
      <div className="space-y-4">
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">Theme</h3>
          <p className="text-ink-dull text-xs">Choose your preferred theme</p>
        </div>
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">Language</h3>
          <p className="text-ink-dull text-xs">Select your language</p>
        </div>
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="mb-1 font-medium text-ink text-sm">
                Reset All Data
              </h3>
              <p className="text-ink-dull text-xs">
                Permanently delete all libraries and settings
              </p>
            </div>
            <button
              className="rounded-lg bg-red-600 px-4 py-2 font-medium text-sm text-white transition-colors hover:bg-red-700 disabled:opacity-50"
              disabled={resetData.isPending}
              onClick={handleResetData}
            >
              {resetData.isPending ? "Resetting..." : "Reset"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function LibrarySettings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="mb-2 font-semibold text-ink text-lg">Library</h2>
        <p className="text-ink-dull text-sm">
          Manage your Spacedrive libraries.
        </p>
      </div>
      <div className="space-y-4">
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">Current Library</h3>
          <p className="text-ink-dull text-xs">View and switch libraries</p>
        </div>
      </div>
    </div>
  );
}

function PrivacySettings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="mb-2 font-semibold text-ink text-lg">Privacy</h2>
        <p className="text-ink-dull text-sm">
          Control your privacy and data sharing preferences.
        </p>
      </div>
      <div className="space-y-4">
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">Telemetry</h3>
          <p className="text-ink-dull text-xs">
            Help improve Spacedrive by sharing anonymous usage data
          </p>
        </div>
      </div>
    </div>
  );
}

function AboutSettings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="mb-2 font-semibold text-ink text-lg">About</h2>
        <p className="text-ink-dull text-sm">Information about Spacedrive.</p>
      </div>
      <div className="space-y-4">
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">Version</h3>
          <p className="text-ink-dull text-xs">Spacedrive v0.1.0</p>
        </div>
        <div className="rounded-lg border border-app-line bg-app-box p-4">
          <h3 className="mb-1 font-medium text-ink text-sm">License</h3>
          <p className="text-ink-dull text-xs">AGPL-3.0</p>
        </div>
      </div>
    </div>
  );
}

export function Settings() {
  const pathname = window.location.pathname;
  const initialPage = pathname.split("/").filter(Boolean)[1] || "general";
  const [currentPage, setCurrentPage] = useState(initialPage);

  return (
    <div className="flex h-screen bg-app">
      {/* Sidebar */}
      <nav className="w-48 border-sidebar-line border-r bg-sidebar p-4">
        <div className="mb-6">
          <h1 className="font-semibold text-sidebar-ink text-xl">Settings</h1>
        </div>
        <SettingsSidebar
          currentPage={currentPage}
          onPageChange={setCurrentPage}
        />
      </nav>

      {/* Main content */}
      <main className="flex-1 overflow-auto p-8">
        <SettingsContent page={currentPage} />
      </main>
    </div>
  );
}
