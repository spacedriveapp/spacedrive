import {
  Camera,
  ChartPieSlice,
  Clock,
  Columns,
  GridFour,
  Rows,
  Sparkle,
  SquaresFour,
} from "@phosphor-icons/react";
import { TopBarButton } from "@sd/ui";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type ViewMode = "list" | "grid" | "column" | "media" | "size" | "knowledge";

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
    color: "bg-accent",
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
    id: "knowledge",
    label: "Knowledge",
    icon: Sparkle,
    color: "bg-purple-500",
    keybind: "⌘6",
  },
  {
    id: "timeline",
    label: "Timeline",
    icon: Clock,
    color: "bg-yellow-500",
    keybind: "⌘7",
  },
];

interface ViewModeMenuPanelProps {
  viewMode: ViewMode;
  onViewModeChange: (mode: ViewMode) => void;
  onClose?: () => void;
}

export function ViewModeMenuPanel({
  viewMode,
  onViewModeChange,
  onClose,
}: ViewModeMenuPanelProps) {
  const availableViews = viewOptions.filter(
    (option) => option.id !== "knowledge" || import.meta.env.DEV
  );

  return (
    <div className="w-[240px] rounded-lg border border-app-line bg-app p-2 shadow-2xl">
      <div className="grid grid-cols-3 gap-1">
        {availableViews.map((option) => (
          <button
            className={clsx(
              "flex flex-col items-center gap-1.5 rounded-md px-2 py-2",
              option.id === "timeline" && "cursor-not-allowed opacity-50",
              viewMode === option.id
                ? "bg-app-selected"
                : "hover:bg-app-hover/50"
            )}
            key={`${option.id}-${option.label}`}
            onClick={() => {
              if (option.id !== "timeline") {
                onViewModeChange(option.id as ViewMode);
              }
              onClose?.();
            }}
          >
            <option.icon
              className="size-6 text-white"
              weight={viewMode === option.id ? "fill" : "bold"}
            />
            <div className="flex flex-col items-center gap-0.5">
              <div className="font-medium text-menu-ink text-xs">
                {option.label}
              </div>
              <div className="text-[10px] text-menu-faint">
                {option.keybind}
              </div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

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
        active={isOpen}
        icon={SquaresFour}
        onClick={() => setIsOpen(!isOpen)}
        ref={buttonRef}
      >
        Views
      </TopBarButton>

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
              <ViewModeMenuPanel
                onClose={() => setIsOpen(false)}
                onViewModeChange={onViewModeChange}
                viewMode={viewMode}
              />
            </motion.div>
          </AnimatePresence>,
          document.body
        )}
    </>
  );
}
