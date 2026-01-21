import clsx from "clsx";
import { motion } from "framer-motion";
import type { Icon } from "@phosphor-icons/react";
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
    <div className={clsx("flex gap-0.5 p-0.5 bg-app-box/50 rounded-lg", className)}>
      {tabs.map((tab) => {
        const Icon = tab.icon;
        const isActive = activeTab === tab.id;
        const isHovered = hoveredTab === tab.id;

        return (
          <div key={tab.id} className="relative">
            <button
              onClick={() => onChange(tab.id)}
              onMouseEnter={() => setHoveredTab(tab.id)}
              onMouseLeave={() => setHoveredTab(null)}
              className={clsx(
                "relative p-2 rounded-md transition-all",
                "focus:outline-none focus:ring-1 focus:ring-accent",
                isActive
                  ? "text-sidebar-ink"
                  : "text-sidebar-inkDull hover:text-sidebar-ink",
              )}
              title={tab.label}
            >
              {isActive && (
                <motion.div
                  layoutId="activeTab"
                  className="absolute inset-0 bg-sidebar-selected/60 rounded-md"
                  transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
                />
              )}
              <div className="relative z-10 flex items-center justify-center">
                <Icon className="size-4" weight="bold" />
                {tab.badge !== undefined && tab.badge > 0 && (
                  <span className="absolute -top-1.5 -right-1.5 px-1 min-w-[16px] h-4 flex items-center justify-center text-[9px] font-bold bg-accent text-white rounded-full">
                    {tab.badge}
                  </span>
                )}
              </div>
            </button>

            {/* Tooltip */}
            {isHovered && !isActive && (
              <motion.div
                initial={{ opacity: 0, y: -5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.3 }}
                className="absolute top-full left-1/2 -translate-x-1/2 mt-1 px-2 py-1 bg-app-box border border-app-line rounded-md shadow-lg pointer-events-none z-50 whitespace-nowrap"
              >
                <span className="text-xs text-sidebar-ink font-medium">
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