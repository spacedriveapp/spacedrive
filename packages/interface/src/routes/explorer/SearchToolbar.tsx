import { FunnelSimple, X } from "@phosphor-icons/react";
import clsx from "clsx";
import type { SearchScope } from "./context";
import { useExplorer } from "./context";

export function SearchToolbar() {
  const explorer = useExplorer();

  if (explorer.mode.type !== "search") {
    return null;
  }

  const { scope } = explorer.mode;

  const handleScopeChange = (newScope: SearchScope) => {
    if (explorer.mode.type === "search") {
      explorer.enterSearchMode(explorer.mode.query, newScope);
    }
  };

  return (
    <div className="flex items-center gap-3 border-sidebar-line/30 border-b bg-sidebar-box/10 px-4 py-2">
      <div className="flex items-center gap-2">
        <span className="font-medium text-sidebar-inkDull text-xs">
          Search in:
        </span>
        <div className="flex items-center gap-1 rounded-lg bg-sidebar-box/30 p-0.5">
          <ScopeButton
            active={scope === "folder"}
            onClick={() => handleScopeChange("folder")}
          >
            This Folder
          </ScopeButton>
          <ScopeButton
            active={scope === "location"}
            onClick={() => handleScopeChange("location")}
          >
            Location
          </ScopeButton>
          <ScopeButton
            active={scope === "library"}
            onClick={() => handleScopeChange("library")}
          >
            Library
          </ScopeButton>
        </div>
      </div>

      <div className="h-4 w-px bg-sidebar-line/30" />

      <button
        className={clsx(
          "flex items-center gap-1.5 rounded-md px-2 py-1",
          "font-medium text-sidebar-ink text-xs",
          "transition-colors hover:bg-sidebar-selected/40"
        )}
      >
        <FunnelSimple className="size-3.5" weight="bold" />
        Filters
      </button>

      <div className="flex-1" />

      <button
        className={clsx(
          "flex items-center gap-1.5 rounded-md px-2 py-1",
          "font-medium text-sidebar-inkDull text-xs",
          "transition-colors hover:bg-sidebar-selected/40 hover:text-sidebar-ink"
        )}
        onClick={explorer.exitSearchMode}
      >
        <X className="size-3.5" weight="bold" />
        Clear Search
      </button>
    </div>
  );
}

interface ScopeButtonProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function ScopeButton({ active, onClick, children }: ScopeButtonProps) {
  return (
    <button
      className={clsx(
        "rounded-md px-3 py-1 font-medium text-xs transition-all",
        active
          ? "bg-accent text-white shadow-sm"
          : "text-sidebar-inkDull hover:bg-sidebar-selected/30 hover:text-sidebar-ink"
      )}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
