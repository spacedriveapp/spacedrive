import { motion } from "framer-motion";
import type { ReactNode } from "react";

interface ShinyToggleOption<T extends string> {
  value: T;
  label: ReactNode;
  count?: number;
}

interface ShinyToggleProps<T extends string> {
  value: T;
  onChange: (value: T) => void;
  options: ShinyToggleOption<T>[];
  className?: string;
}

export function ShinyToggle<T extends string>({
  value,
  onChange,
  options,
  className = "",
}: ShinyToggleProps<T>) {
  return (
    <div
      className={`inline-flex gap-1 rounded-full border border-app-line bg-app-box p-1 ${className}`}
    >
      {options.map((option) => (
        <button
          className="relative px-3 py-1.5 font-medium text-sm transition-colors"
          key={option.value}
          onClick={() => onChange(option.value)}
          type="button"
        >
          {value === option.value && (
            <motion.div
              className="absolute inset-0 rounded-full border-2 border-[#42B2FD]/40 bg-gradient-to-b from-[#42B2FD]/80 to-[#0078F0]/80 shadow-[0_0_0.75rem_hsl(207_100%_65%/30%)]"
              layoutId="shinyToggleActive"
              transition={{ type: "spring", bounce: 0.2, duration: 0.6 }}
            />
          )}
          <span
            className={`relative ${value === option.value ? "text-white" : "text-ink-dull"}`}
          >
            {option.label}
            {option.count !== undefined && ` (${option.count})`}
          </span>
        </button>
      ))}
    </div>
  );
}
