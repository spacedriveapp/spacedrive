import { Plus, X } from "@phosphor-icons/react";
import clsx from "clsx";
import { LayoutGroup, motion } from "framer-motion";
import { useMemo } from "react";
import { useTabManager } from "./useTabManager";

export function TabBar() {
  const { tabs, activeTabId, switchTab, closeTab, createTab } = useTabManager();

  // Don't show tab bar if only one tab
  if (tabs.length <= 1) {
    return null;
  }

  // Ensure activeTabId exists in tabs array, fallback to first tab
  // Memoize to prevent unnecessary rerenders during rapid state updates
  const safeActiveTabId = useMemo(() => {
    return tabs.find((t) => t.id === activeTabId)?.id ?? tabs[0]?.id;
  }, [tabs, activeTabId]);

  return (
    <div className="mx-2 flex h-9 shrink-0 items-center gap-1 rounded-full bg-app-box/50 px-1">
      <LayoutGroup id="tab-bar">
        <div className="flex min-w-0 flex-1 items-center gap-1">
          {tabs.map((tab) => {
            const isActive = tab.id === safeActiveTabId;

            return (
              <button
                className={clsx(
                  "group relative flex min-w-0 flex-1 items-center justify-center rounded-full py-1.5 text-[13px]",
                  isActive
                    ? "text-ink"
                    : "text-ink-dull hover:bg-app-hover/50 hover:text-ink"
                )}
                key={tab.id}
                onClick={() => switchTab(tab.id)}
              >
                {isActive && (
                  <motion.div
                    className="absolute inset-0 rounded-full bg-app-selected shadow-sm"
                    initial={false}
                    layoutId="activeTab"
                    transition={{
                      type: "spring",
                      stiffness: 500,
                      damping: 35,
                    }}
                  />
                )}
                {/* Close button - absolutely positioned left */}
                <span
                  className={clsx(
                    "absolute left-1.5 z-10 flex size-5 cursor-pointer items-center justify-center rounded-full transition-all",
                    isActive
                      ? "opacity-60 hover:bg-app-hover hover:opacity-100"
                      : "hover:!opacity-100 opacity-0 hover:bg-app-hover group-hover:opacity-60"
                  )}
                  onClick={(e) => {
                    e.stopPropagation();
                    closeTab(tab.id);
                  }}
                  title="Close tab"
                >
                  <X size={10} weight="bold" />
                </span>
                <span className="relative z-10 truncate px-6">{tab.title}</span>
              </button>
            );
          })}
        </div>
      </LayoutGroup>
      <button
        className="flex size-7 shrink-0 items-center justify-center rounded-full text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
        onClick={() => createTab()}
        title="New tab (⌘T)"
      >
        <Plus size={14} weight="bold" />
      </button>
    </div>
  );
}
