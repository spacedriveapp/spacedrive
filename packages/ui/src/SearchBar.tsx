import { MagnifyingGlass, X } from "@phosphor-icons/react";
import clsx from "clsx";
import { forwardRef, useState } from "react";

interface SearchBarProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange'> {
	value?: string;
	onChange?: (value: string) => void;
	onClear?: () => void;
}

export const SearchBar = forwardRef<HTMLInputElement, SearchBarProps>(
	({ value, onChange, onClear, className, placeholder = "Search...", ...props }, ref) => {
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
					"flex items-center h-8 px-3 gap-2",
					"rounded-full backdrop-blur-xl",
					"border border-sidebar-line/30 bg-sidebar-box/20",
					"transition-all focus-within:bg-sidebar-box/30",
					className
				)}
			>
				<MagnifyingGlass className="size-[18px] text-sidebar-inkFaint flex-shrink-0" weight="bold" />
				<input
					ref={ref}
					type="text"
					value={currentValue}
					onChange={handleChange}
					placeholder={placeholder}
					className={clsx(
						"flex-1 bg-transparent outline-none border-0 p-0",
						"text-xs font-medium text-sidebar-ink placeholder:text-sidebar-inkFaint",
						"min-w-0",
						"focus:outline-none focus:ring-0 focus:border-0"
					)}
					{...props}
				/>
				{currentValue && (
					<button
						onClick={handleClear}
						className="flex-shrink-0 p-0.5 rounded-full hover:bg-sidebar-selected/40 transition-colors"
					>
						<X className="size-3 text-sidebar-inkDull" weight="bold" />
					</button>
				)}
			</div>
		);
	}
);

SearchBar.displayName = "SearchBar";