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
	Sparkle,
} from "@phosphor-icons/react";
import clsx from "clsx";
import { TopBarButton } from "@sd/ui";

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

	// Filter out knowledge view in production
	const availableViews = viewOptions.filter(
		(option) => option.id !== "knowledge" || import.meta.env.DEV
	);

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
							className="w-[240px] rounded-lg bg-app border border-app-line shadow-2xl p-2 z-50"
						>
							<div className="grid grid-cols-3 gap-1">
								{availableViews.map((option) => (
									<button
										key={`${option.id}-${option.label}`}
										onClick={() => {
											if (option.id !== "timeline") {
												onViewModeChange(
													option.id as ViewMode,
												);
											}
											setIsOpen(false);
										}}
										className={clsx(
											"flex flex-col items-center gap-1.5 px-2 py-2 rounded-md",

											option.id === "timeline" &&
												"opacity-50 cursor-not-allowed",
											viewMode === option.id
												? "bg-app-selected"
												: "hover:bg-app-hover/50",
										)}
									>
										<option.icon
											className="size-6 text-white"
											weight={
												viewMode === option.id
													? "fill"
													: "bold"
											}
										/>
										<div className="flex flex-col items-center gap-0.5">
											<div className="text-xs font-medium text-menu-ink">
												{option.label}
											</div>
											<div className="text-[10px] text-menu-faint">
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
