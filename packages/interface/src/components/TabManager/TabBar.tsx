import clsx from "clsx";
import { motion, LayoutGroup } from "framer-motion";
import { Plus, X } from "@phosphor-icons/react";
import { useTabManager } from "./useTabManager";
import { useMemo } from "react";

export function TabBar() {
	const { tabs, activeTabId, switchTab, closeTab, createTab } =
		useTabManager();

	// Don't show tab bar if only one tab
	if (tabs.length <= 1) {
		return null;
	}

	// Ensure activeTabId exists in tabs array, fallback to first tab
	// Memoize to prevent unnecessary rerenders during rapid state updates
	const safeActiveTabId = useMemo(() => {
		return tabs.find((t) => t.id === activeTabId)?.id ?? tabs[0]?.id;
	}, [tabs, activeTabId]);

	return (
		<div className="flex items-center h-9 px-1 gap-1 mx-2 bg-app-box/50 rounded-full shrink-0">
			<LayoutGroup id="tab-bar">
				<div className="flex items-center flex-1 gap-1 min-w-0">
					{tabs.map((tab) => {
						const isActive = tab.id === safeActiveTabId;

						return (
							<button
								key={tab.id}
								onClick={() => switchTab(tab.id)}
								className={clsx(
									"relative flex items-center justify-center py-1.5 rounded-full text-[13px] group flex-1 min-w-0",
									isActive
										? "text-ink"
										: "text-ink-dull hover:text-ink hover:bg-app-hover/50",
								)}
							>
								{isActive && (
									<motion.div
										layoutId="activeTab"
										className="absolute inset-0 bg-app-selected rounded-full shadow-sm"
										initial={false}
										transition={{
											type: "spring",
											stiffness: 500,
											damping: 35,
										}}
									/>
								)}
								{/* Close button - absolutely positioned left */}
								<span
									onClick={(e) => {
										e.stopPropagation();
										closeTab(tab.id);
									}}
									className={clsx(
										"absolute left-1.5 z-10 size-5 flex items-center justify-center rounded-full transition-all cursor-pointer",
										isActive
											? "opacity-60 hover:opacity-100 hover:bg-app-hover"
											: "opacity-0 group-hover:opacity-60 hover:!opacity-100 hover:bg-app-hover",
									)}
									title="Close tab"
								>
									<X size={10} weight="bold" />
								</span>
								<span className="relative z-10 truncate px-6">
									{tab.title}
								</span>
							</button>
						);
					})}
				</div>
			</LayoutGroup>
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
