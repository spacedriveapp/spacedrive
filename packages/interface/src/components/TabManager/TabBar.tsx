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
		<div className="flex items-center h-9 px-2 bg-app-box/50 border-b border-app-line shrink-0">
			<div className="flex items-center gap-0.5 overflow-x-auto scrollbar-none">
				{tabs.map((tab) => (
					<motion.button
						key={tab.id}
						layout
						onClick={() => switchTab(tab.id)}
						className={clsx(
							"relative flex items-center gap-1.5 px-3 py-1 rounded-md text-[13px] whitespace-nowrap group min-w-0 max-w-[180px]",
							tab.id === activeTabId
								? "text-ink"
								: "text-ink-dull hover:text-ink hover:bg-app-hover/50",
						)}
					>
						{tab.id === activeTabId && (
							<motion.div
								layoutId="activeTab"
								className="absolute inset-0 bg-app-selected rounded-md shadow-sm"
								transition={{ type: "spring", duration: 0.25, bounce: 0.1 }}
							/>
						)}
						<span className="relative z-10 truncate">{tab.title}</span>
						<button
							onClick={(e) => {
								e.stopPropagation();
								closeTab(tab.id);
							}}
							className={clsx(
								"relative z-10 rounded-sm hover:bg-app-hover p-0.5 shrink-0 transition-opacity",
								tab.id === activeTabId
									? "opacity-60 hover:opacity-100"
									: "opacity-0 group-hover:opacity-60 hover:!opacity-100",
							)}
							title="Close tab"
						>
							<X size={11} weight="bold" />
						</button>
					</motion.button>
				))}
			</div>
			<button
				onClick={() => createTab()}
				className="ml-1 p-1 rounded-md hover:bg-app-hover text-ink-dull hover:text-ink shrink-0 transition-colors"
				title="New tab (⌘T)"
			>
				<Plus size={14} weight="bold" />
			</button>
		</div>
	);
}
