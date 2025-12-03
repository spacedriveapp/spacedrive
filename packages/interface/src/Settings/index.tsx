import { useState } from "react";
import clsx from "clsx";

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
          key={section.id}
          onClick={() => onPageChange(section.id)}
          className={clsx(
            "w-full text-left px-3 py-2 rounded-md text-sm font-medium transition-colors",
            currentPage === section.id
              ? "bg-sidebar-selected text-sidebar-ink"
              : "text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-box"
          )}
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
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">General</h2>
        <p className="text-sm text-ink-dull">
          Configure general application settings.
        </p>
      </div>
      <div className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Theme</h3>
          <p className="text-xs text-ink-dull">Choose your preferred theme</p>
        </div>
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Language</h3>
          <p className="text-xs text-ink-dull">Select your language</p>
        </div>
      </div>
    </div>
  );
}

function LibrarySettings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Library</h2>
        <p className="text-sm text-ink-dull">
          Manage your Spacedrive libraries.
        </p>
      </div>
      <div className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">
            Current Library
          </h3>
          <p className="text-xs text-ink-dull">View and switch libraries</p>
        </div>
      </div>
    </div>
  );
}

function PrivacySettings() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-ink mb-2">Privacy</h2>
        <p className="text-sm text-ink-dull">
          Control your privacy and data sharing preferences.
        </p>
      </div>
      <div className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Telemetry</h3>
          <p className="text-xs text-ink-dull">
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
        <h2 className="text-lg font-semibold text-ink mb-2">About</h2>
        <p className="text-sm text-ink-dull">
          Information about Spacedrive.
        </p>
      </div>
      <div className="space-y-4">
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">Version</h3>
          <p className="text-xs text-ink-dull">Spacedrive v0.1.0</p>
        </div>
        <div className="p-4 bg-app-box rounded-lg border border-app-line">
          <h3 className="text-sm font-medium text-ink mb-1">License</h3>
          <p className="text-xs text-ink-dull">AGPL-3.0</p>
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
    <div className="h-screen bg-app flex">
      {/* Sidebar */}
      <nav className="w-48 bg-sidebar border-r border-sidebar-line p-4">
        <div className="mb-6">
          <h1 className="text-xl font-semibold text-sidebar-ink">Settings</h1>
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
