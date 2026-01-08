import { MagnifyingGlass, X } from "@phosphor-icons/react";
import clsx from "clsx";
import { forwardRef, useState } from "react";

interface SearchBarProps
  extends Omit<
    React.InputHTMLAttributes<HTMLInputElement>,
    "value" | "onChange"
  > {
  value?: string;
  onChange?: (value: string) => void;
  onClear?: () => void;
}

export const SearchBar = forwardRef<HTMLInputElement, SearchBarProps>(
  (
    {
      value,
      onChange,
      onClear,
      className,
      placeholder = "Search...",
      ...props
    },
    ref
  ) => {
    const [internalValue, setInternalValue] = useState("");
    const currentValue = value !== undefined ? value : internalValue;

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = e.target.value;
      if (onChange) {
        onChange(newValue);
      } else {
        setInternalValue(newValue);
      }
    };

    const handleClear = () => {
      if (onChange) {
        onChange("");
      } else {
        setInternalValue("");
      }
      onClear?.();
    };

    return (
      <div
        className={clsx(
          "flex h-8 items-center gap-2 px-3",
          "rounded-full backdrop-blur-xl",
          "border border-sidebar-line/30 bg-sidebar-box/20",
          "transition-all focus-within:bg-sidebar-box/30",
          className
        )}
      >
        <MagnifyingGlass
          className="size-[18px] flex-shrink-0 text-sidebar-inkFaint"
          weight="bold"
        />
        <input
          className={clsx(
            "flex-1 border-0 bg-transparent p-0 outline-none",
            "font-medium text-sidebar-ink text-xs placeholder:text-sidebar-inkFaint",
            "min-w-0",
            "focus:border-0 focus:outline-none focus:ring-0"
          )}
          onChange={handleChange}
          placeholder={placeholder}
          ref={ref}
          type="text"
          value={currentValue}
          {...props}
        />
        {currentValue && (
          <button
            className="flex-shrink-0 rounded-full p-0.5 transition-colors hover:bg-sidebar-selected/40"
            onClick={handleClear}
          >
            <X className="size-3 text-sidebar-inkDull" weight="bold" />
          </button>
        )}
      </div>
    );
  }
);

SearchBar.displayName = "SearchBar";
