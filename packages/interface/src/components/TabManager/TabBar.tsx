import clsx from "clsx";
import { motion } from "framer-motion";
import { Plus, X } from "@phosphor-icons/react";
import { useTabManager } from "./useTabManager";

export function TabBar() {
	const { tabs, activeTabId, switchTab, closeTab, createTab } =
		useTabManager();

	// Don't show tab bar if only one tab
	if (tabs.length <= 1) {
		return null;
	}

	return (
		<div className="flex items-center h-9 px-1 gap-1 mx-2 mb-1.5 bg-app-box/50 rounded-full shrink-0">
			<div className="flex items-center flex-1 gap-1 min-w-0">
				{tabs.map((tab) => (
					<motion.button
						key={tab.id}
						layout
						onClick={() => switchTab(tab.id)}
						className={clsx(
							"relative flex items-center justify-center py-1.5 rounded-full text-[13px] group flex-1 min-w-0",
							tab.id === activeTabId
								? "text-ink"
								: "text-ink-dull hover:text-ink hover:bg-app-hover/50",
						)}
					>
						{tab.id === activeTabId && (
							<motion.div
								layoutId="activeTab"
								className="absolute inset-0 bg-app-selected rounded-full shadow-sm"
								transition={{
									type: "easeInOut",
									duration: 0.15,
								}}
							/>
						)}
						{/* Close button - absolutely positioned left */}
						<button
							onClick={(e) => {
								e.stopPropagation();
								closeTab(tab.id);
							}}
							className={clsx(
								"absolute left-1.5 z-10 size-5 flex items-center justify-center rounded-full transition-all",
								tab.id === activeTabId
									? "opacity-60 hover:opacity-100 hover:bg-app-hover"
									: "opacity-0 group-hover:opacity-60 hover:!opacity-100 hover:bg-app-hover",
							)}
							title="Close tab"
						>
							<X size={10} weight="bold" />
						</button>
						<span className="relative z-10 truncate px-6">
							{tab.title}
						</span>
					</motion.button>
				))}
			</div>
			<button
				onClick={() => createTab()}
				className="size-7 flex items-center justify-center rounded-full hover:bg-app-hover text-ink-dull hover:text-ink shrink-0 transition-colors"
				title="New tab (⌘T)"
			>
				<Plus size={14} weight="bold" />
			</button>
		</div>
	);
}
