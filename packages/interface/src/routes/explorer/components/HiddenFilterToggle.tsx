import clsx from "clsx";
import { useExplorer, type HiddenFilter } from "../context";

const OPTIONS: { value: HiddenFilter; label: string; title: string }[] = [
	{
		value: "regular",
		label: "Regular",
		title: "Show only regular files (hide hidden)",
	},
	{
		value: "all",
		label: "All",
		title: "Show all files, including hidden",
	},
	{
		value: "hidden",
		label: "Hidden",
		title: "Show only hidden files and folders",
	},
];

/**
 * Three-section segmented control for choosing which files are listed:
 * Regular / All / Hidden. Styled to sit inline in the explorer top bar.
 */
export function HiddenFilterToggle() {
	const { hiddenFilter, setHiddenFilter } = useExplorer();

	return (
		<div
			role="radiogroup"
			aria-label="Hidden files"
			className="flex items-center gap-0.5 rounded-full border border-app-line bg-app-box/60 p-0.5"
		>
			{OPTIONS.map((opt) => {
				const active = hiddenFilter === opt.value;
				return (
					<button
						key={opt.value}
						type="button"
						role="radio"
						aria-checked={active}
						title={opt.title}
						onClick={() => setHiddenFilter(opt.value)}
						className={clsx(
							"rounded-full px-2.5 py-1 text-[11px] font-medium leading-none transition-colors",
							active
								? "bg-accent text-white shadow-sm"
								: "text-ink-dull hover:bg-app-hover hover:text-ink",
						)}
					>
						{opt.label}
					</button>
				);
			})}
		</div>
	);
}
