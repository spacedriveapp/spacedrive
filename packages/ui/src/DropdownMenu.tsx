"use client";

import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import {
  type PropsWithChildren,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";

// Minimal base styles - customize via className prop
const baseMenuStyles = "overflow-hidden w-full";
const baseItemWrapperStyles = "w-full";
const baseItemStyles = "flex w-full items-center cursor-pointer";
const baseSeparatorStyles = "border-b";

// ===== ROOT COMPONENT =====
interface DropdownMenuProps {
  trigger: ReactNode;
  className?: string;
  onOpenChange?: (open: boolean) => void;
}

function Root({
  onOpenChange,
  trigger,
  className,
  children,
}: PropsWithChildren<DropdownMenuProps>) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleOpenChange = (open: boolean) => {
    setIsOpen(open);
    onOpenChange?.(open);
  };

  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        handleOpenChange(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  return (
    <div className="w-full" ref={containerRef}>
      <div onClick={() => handleOpenChange(!isOpen)}>{trigger}</div>

      <AnimatePresence>
        {isOpen && (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            className="overflow-hidden"
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15, ease: [0.25, 1, 0.5, 1] }}
          >
            <div className={clsx(baseMenuStyles, "mt-1", className)}>
              {children}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ===== ITEM COMPONENT =====
interface ItemProps {
  icon?: React.ElementType;
  label?: string;
  children?: ReactNode;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

function Item({
  icon: Icon,
  label,
  children,
  selected,
  className,
  onClick,
}: ItemProps) {
  return (
    <div className={baseItemWrapperStyles}>
      <button className={clsx(baseItemStyles, className)} onClick={onClick}>
        {Icon && <Icon className="mr-2 size-4" weight="bold" />}
        {label || children}
      </button>
    </div>
  );
}

// ===== SEPARATOR COMPONENT =====
function Separator({ className }: { className?: string }) {
  return <div className={clsx(baseSeparatorStyles, className)} />;
}

export const DropdownMenu = {
  Root,
  Item,
  Separator,
};
