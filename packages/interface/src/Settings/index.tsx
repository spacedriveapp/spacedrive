import { useState } from "react";
import clsx from "clsx";
import {
  GeneralSettings,
  AppearanceSettings,
  LibrarySettings,
  IndexerSettings,
  ServicesSettings,
  PrivacySettings,
  AdvancedSettings,
  AboutSettings,
} from "./pages";

interface SettingsSidebarProps {
  currentPage: string;
  onPageChange: (page: string) => void;
}

const sections = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "library", label: "Library" },
  { id: "indexer", label: "Indexer" },
  { id: "services", label: "Services" },
  { id: "privacy", label: "Privacy" },
  { id: "advanced", label: "Advanced" },
  { id: "about", label: "About" },
];

function SettingsSidebar({ currentPage, onPageChange }: SettingsSidebarProps) {
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
    case "appearance":
      return <AppearanceSettings />;
    case "library":
      return <LibrarySettings />;
    case "indexer":
      return <IndexerSettings />;
    case "services":
      return <ServicesSettings />;
    case "privacy":
      return <PrivacySettings />;
    case "advanced":
      return <AdvancedSettings />;
    case "about":
      return <AboutSettings />;
    default:
      return <GeneralSettings />;
  }
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
