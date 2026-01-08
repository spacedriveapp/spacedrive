import {
  CalendarBlank,
  Camera,
  Check,
  FileText,
  Ruler,
  SortAscending,
  TextAa,
} from "@phosphor-icons/react";
import type { DirectorySortBy, MediaSortBy } from "@sd/ts-client";
import { TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface SortMenuPanelProps {
  sortBy: DirectorySortBy | MediaSortBy;
  onSortChange: (sort: DirectorySortBy | MediaSortBy) => void;
  viewMode: "grid" | "list" | "media" | "column";
}

export function SortMenuPanel({
  sortBy,
  onSortChange,
  viewMode,
}: SortMenuPanelProps) {
  const sortOptions =
    viewMode === "media"
      ? [
          { value: "datetaken", label: "Date Taken", icon: Camera },
          { value: "modified", label: "Date Modified", icon: CalendarBlank },
          { value: "created", label: "Date Created", icon: CalendarBlank },
          { value: "name", label: "Name", icon: TextAa },
          { value: "size", label: "Size", icon: Ruler },
        ]
      : [
          { value: "name", label: "Name", icon: TextAa },
          { value: "modified", label: "Date Modified", icon: CalendarBlank },
          { value: "size", label: "Size", icon: Ruler },
          { value: "type", label: "Type", icon: FileText },
        ];

  return (
    <div className="w-56 overflow-hidden rounded-lg border border-app-line bg-app-box shadow-lg">
      <div className="border-app-line border-b px-3 py-2 font-semibold text-sidebar-ink text-xs uppercase tracking-wider">
        Sort By
      </div>

      <div className="py-1">
        {sortOptions.map((option) => {
          const Icon = option.icon;
          const isActive = sortBy === option.value;

          return (
            <button
              className={clsx(
                "flex w-full items-center gap-2.5 px-3 py-2 text-sm transition-colors hover:bg-app-hover",
                isActive ? "text-accent" : "text-ink"
              )}
              key={option.value}
              onClick={() => {
                onSortChange(option.value as DirectorySortBy | MediaSortBy);
              }}
            >
              <Icon className="size-4" weight="bold" />
              <span className="flex-1 text-left">{option.label}</span>
              {isActive && <Check className="size-4" weight="bold" />}
            </button>
          );
        })}
      </div>
    </div>
  );
}

interface SortMenuProps {
  sortBy: DirectorySortBy | MediaSortBy;
  onSortChange: (sort: DirectorySortBy | MediaSortBy) => void;
  viewMode: "grid" | "list" | "media" | "column";
  className?: string;
}

export function SortMenu({
  sortBy,
  onSortChange,
  viewMode,
  className,
}: SortMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, right: 0 });

  // Update position when opened
  useEffect(() => {
    if (isOpen && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8,
        right: window.innerWidth - rect.right,
      });
    }
  }, [isOpen]);

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        panelRef.current &&
        buttonRef.current &&
        !panelRef.current.contains(e.target as Node) &&
        !buttonRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () =>
        document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  return (
    <>
      <div className={clsx(className)}>
        <TopBarButton
          active={isOpen}
          icon={SortAscending}
          onClick={() => setIsOpen(!isOpen)}
          ref={buttonRef}
          title="Sort"
        />
      </div>

      {isOpen &&
        createPortal(
          <AnimatePresence>
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              className="z-50"
              exit={{ opacity: 0, y: -10 }}
              initial={{ opacity: 0, y: -10 }}
              ref={panelRef}
              style={{
                position: "fixed",
                top: `${position.top}px`,
                right: `${position.right}px`,
              }}
              transition={{ duration: 0.15 }}
            >
              <SortMenuPanel
                onSortChange={onSortChange}
                sortBy={sortBy}
                viewMode={viewMode}
              />
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </>
  );
}
