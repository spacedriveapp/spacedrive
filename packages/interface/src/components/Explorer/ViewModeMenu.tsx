import { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  Rows,
  GridFour,
  Camera,
  Columns,
  ChartPieSlice,
  Clock,
  SquaresFour,
} from "@phosphor-icons/react";
import clsx from "clsx";
import { TopBarButton } from "@sd/ui";

type ViewMode = "list" | "grid" | "column" | "media" | "size";

interface ViewOption {
  id: ViewMode | "timeline";
  label: string;
  icon: React.ElementType;
  color: string;
  keybind: string;
}

const viewOptions: ViewOption[] = [
  {
    id: "grid",
    label: "Grid",
    icon: GridFour,
    color: "bg-blue-500",
    keybind: "⌘1",
  },
  {
    id: "list",
    label: "List",
    icon: Rows,
    color: "bg-purple-500",
    keybind: "⌘2",
  },
  {
    id: "media",
    label: "Media",
    icon: Camera,
    color: "bg-pink-500",
    keybind: "⌘3",
  },
  {
    id: "column",
    label: "Column",
    icon: Columns,
    color: "bg-orange-500",
    keybind: "⌘4",
  },
  {
    id: "size",
    label: "Size",
    icon: ChartPieSlice,
    color: "bg-green-500",
    keybind: "⌘5",
  },
  {
    id: "timeline",
    label: "Timeline",
    icon: Clock,
    color: "bg-yellow-500",
    keybind: "⌘6",
  },
];

interface ViewModeMenuProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
}

export function ViewModeMenu({
  viewMode,
  onViewModeChange,
}: ViewModeMenuProps) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ top: 0, right: 0 });

  useEffect(() => {
    if (isOpen && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      setPosition({
        top: rect.bottom + 8,
        right: window.innerWidth - rect.right,
      });
    }
  }, [isOpen]);

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
      <TopBarButton
        ref={buttonRef}
        icon={SquaresFour}
        onClick={() => setIsOpen(!isOpen)}
        active={isOpen}
      >
        Views
      </TopBarButton>

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
              className="w-[280px] rounded-lg bg-menu border border-menu-line shadow-2xl p-2 z-50"
            >
              <div className="grid grid-cols-2 gap-1.5">
                {viewOptions.map((option) => (
                  <button
                    key={`${option.id}-${option.label}`}
                    onClick={() => {
                      if (option.id !== "timeline") {
                        onViewModeChange(option.id as ViewMode);
                      }
                      setIsOpen(false);
                    }}
                    className={clsx(
                      "flex items-center gap-3 px-2.5 py-2 rounded-md",
                      "transition-colors text-left",
                      option.id === "timeline" &&
                        "opacity-50 cursor-not-allowed",
                      viewMode === option.id
                        ? "bg-menu-selected"
                        : "hover:bg-menu-hover",
                    )}
                  >
                    <div
                      className={clsx(
                        "flex items-center justify-center size-8 rounded-md",
                        option.color,
                      )}
                    >
                      <option.icon
                        className="size-4 text-white"
                        weight="bold"
                      />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium text-menu-ink">
                        {option.label}
                      </div>
                      <div className="text-[11px] text-menu-faint">
                        {option.keybind}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </motion.div>
          </AnimatePresence>,
          document.body,
        )}
    </>
  );
}
