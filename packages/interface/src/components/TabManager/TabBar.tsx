import clsx from "clsx";
import { motion } from "framer-motion";
import { Plus, X } from "@phosphor-icons/react";
import { useTabManager } from "./useTabManager";

export function TabBar() {
	const { tabs, activeTabId, switchTab, closeTab, createTab } =
		useTabManager();

	return (
		<div className="flex items-center gap-1 px-2 py-1 bg-app border-b border-app-line overflow-x-auto">
			{tabs.map((tab) => (
				<motion.button
					key={tab.id}
					layout
					onClick={() => switchTab(tab.id)}
					className={clsx(
						"relative flex items-center gap-2 px-3 py-1.5 rounded-md text-sm whitespace-nowrap group",
						tab.id === activeTabId
							? "text-sidebar-ink"
							: "text-sidebar-inkDull hover:text-sidebar-ink",
					)}
				>
					{tab.id === activeTabId && (
						<motion.div
							layoutId="activeTab"
							className="absolute inset-0 bg-sidebar-selected/60 rounded-md"
							transition={{ type: "spring", duration: 0.3 }}
						/>
					)}
					<span className="relative z-10">{tab.title}</span>
					{tabs.length > 1 && (
						<button
							onClick={(e) => {
								e.stopPropagation();
								closeTab(tab.id);
							}}
							className={clsx(
								"relative z-10 rounded-sm hover:bg-app-selected/60 p-0.5",
								tab.id === activeTabId
									? "opacity-70"
									: "opacity-0 group-hover:opacity-70",
							)}
							title="Close tab"
						>
							<X size={12} weight="bold" />
						</button>
					)}
				</motion.button>
			))}
			<button
				onClick={() => createTab()}
				className="px-2 py-1 rounded-md hover:bg-app-hover text-ink-dull"
				title="New tab"
			>
				<Plus size={16} weight="bold" />
			</button>
		</div>
	);
}
