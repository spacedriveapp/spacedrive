import { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import {
  SortAscending,
  TextAa,
  CalendarBlank,
  Ruler,
  FileText,
  Check,
  Camera,
} from "@phosphor-icons/react";
import { motion, AnimatePresence } from "framer-motion";
import clsx from "clsx";
import { TopBarButton } from "@sd/ui";
import type { DirectorySortBy, MediaSortBy } from "@sd/ts-client";

interface SortMenuProps {
  sortBy: DirectorySortBy | MediaSortBy;
  onSortChange: (sort: DirectorySortBy | MediaSortBy) => void;
  viewMode: "grid" | "list" | "media" | "column";
  className?: string;
}

export function SortMenu({ sortBy, onSortChange, viewMode, className }: SortMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, right: 0 });

  // Different sort options based on view mode
  const sortOptions = viewMode === "media"
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
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [isOpen]);

  return (
    <>
      <div className={clsx(className)}>
        <TopBarButton
          ref={buttonRef}
          icon={SortAscending}
          onClick={() => setIsOpen(!isOpen)}
          active={isOpen}
          title="Sort"
        />
      </div>

      {isOpen &&
        createPortal(
          <AnimatePresence>
            <motion.div
              ref={panelRef}
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.15 }}
              style={{
                position: "fixed",
                top: `${position.top}px`,
                right: `${position.right}px`,
              }}
              className="w-56 bg-app-box border border-app-line rounded-lg shadow-lg overflow-hidden z-50"
            >
              <div className="px-3 py-2 text-xs font-semibold text-sidebar-ink uppercase tracking-wider border-b border-app-line">
                Sort By
              </div>

              <div className="py-1">
                {sortOptions.map((option) => {
                  const Icon = option.icon;
                  const isActive = sortBy === option.value;

                  return (
                    <button
                      key={option.value}
                      onClick={() => {
                        onSortChange(option.value as DirectorySortBy | MediaSortBy);
                      }}
                      className={clsx(
                        "flex items-center gap-2.5 w-full px-3 py-2 hover:bg-app-hover transition-colors text-sm",
                        isActive ? "text-accent" : "text-ink"
                      )}
                    >
                      <Icon className="size-4" weight="bold" />
                      <span className="flex-1 text-left">{option.label}</span>
                      {isActive && <Check className="size-4" weight="bold" />}
                    </button>
                  );
                })}
              </div>
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </>
  );
}
