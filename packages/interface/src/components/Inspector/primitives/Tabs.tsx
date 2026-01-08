import type { Icon } from "@phosphor-icons/react";
import clsx from "clsx";
import { motion } from "framer-motion";
import { useState } from "react";

interface Tab {
  id: string;
  label: string;
  icon: Icon;
  badge?: number;
}

interface TabsProps {
  tabs: Tab[];
  activeTab: string;
  onChange: (tabId: string) => void;
  className?: string;
}

export function Tabs({ tabs, activeTab, onChange, className }: TabsProps) {
  const [hoveredTab, setHoveredTab] = useState<string | null>(null);

  return (
    <div
      className={clsx("flex gap-0.5 rounded-lg bg-app-box/50 p-0.5", className)}
    >
      {tabs.map((tab) => {
        const Icon = tab.icon;
        const isActive = activeTab === tab.id;
        const isHovered = hoveredTab === tab.id;

        return (
          <div className="relative" key={tab.id}>
            <button
              className={clsx(
                "relative rounded-md p-2 transition-all",
                "focus:outline-none focus:ring-1 focus:ring-accent",
                isActive
                  ? "text-sidebar-ink"
                  : "text-sidebar-inkDull hover:text-sidebar-ink"
              )}
              onClick={() => onChange(tab.id)}
              onMouseEnter={() => setHoveredTab(tab.id)}
              onMouseLeave={() => setHoveredTab(null)}
              title={tab.label}
            >
              {isActive && (
                <motion.div
                  className="absolute inset-0 rounded-md bg-sidebar-selected/60"
                  layoutId="activeTab"
                  transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
                />
              )}
              <div className="relative z-10 flex items-center justify-center">
                <Icon className="size-4" weight="bold" />
                {tab.badge !== undefined && tab.badge > 0 && (
                  <span className="absolute -top-1.5 -right-1.5 flex h-4 min-w-[16px] items-center justify-center rounded-full bg-accent px-1 font-bold text-[9px] text-white">
                    {tab.badge}
                  </span>
                )}
              </div>
            </button>

            {/* Tooltip */}
            {isHovered && !isActive && (
              <motion.div
                animate={{ opacity: 1, y: 0 }}
                className="pointer-events-none absolute top-full left-1/2 z-50 mt-1 -translate-x-1/2 whitespace-nowrap rounded-md border border-app-line bg-app-box px-2 py-1 shadow-lg"
                initial={{ opacity: 0, y: -5 }}
                transition={{ delay: 0.3 }}
              >
                <span className="font-medium text-sidebar-ink text-xs">
                  {tab.label}
                </span>
              </motion.div>
            )}
          </div>
        );
      })}
    </div>
  );
}
