import { motion, AnimatePresence } from "framer-motion";
import { X, Plus } from "@phosphor-icons/react";
import { useDraggable } from "@dnd-kit/core";
import clsx from "clsx";
import type { ItemType, SpaceItem as SpaceItemType, GroupType } from "@sd/ts-client";
import { SpaceItem } from "./SpaceItem";
import { createPortal } from "react-dom";
import { useState } from "react";
import { useLibraryMutation } from "../../context";
import { Input } from "@sd/ui";

interface PaletteItem {
	type: ItemType;
	label: string;
}

const PALETTE_ITEMS: PaletteItem[] = [
	{
		type: "Overview",
		label: "Overview",
	},
	{
		type: "Recents",
		label: "Recents",
	},
	{
		type: "Favorites",
		label: "Favorites",
	},
	{
		type: "FileKinds",
		label: "File Kinds",
	},
];

function DraggablePaletteItem({ item }: { item: PaletteItem }) {
	const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
		id: `palette-${item.label}`,
		data: {
			type: "palette-item",
			itemType: item.type,
		},
	});

	// Create a mock SpaceItem for rendering
	const mockSpaceItem: SpaceItemType = {
		id: `palette-${item.label}`,
		space_id: "",
		group_id: null,
		item_type: item.type,
		order: 0,
		created_at: new Date().toISOString(),
	};

	return (
		<div
			ref={setNodeRef}
			{...attributes}
			{...listeners}
			className={clsx(
				"cursor-move transition-opacity",
				isDragging && "opacity-50",
			)}
		>
			<SpaceItem
				item={mockSpaceItem}
				allowInsertion={false}
				onClick={(e) => {
					e.preventDefault();
					e.stopPropagation();
				}}
			/>
		</div>
	);
}

interface SpaceCustomizationPanelProps {
	isOpen: boolean;
	onClose: () => void;
	spaceId: string | null;
}

function getDefaultGroupName(groupType: GroupType): string {
	if (groupType === "Devices") return "Devices";
	if (groupType === "Locations") return "Locations";
	if (groupType === "Tags") return "Tags";
	if (groupType === "Cloud") return "Cloud";
	if (groupType === "Custom") return "Custom Group";
	if (typeof groupType === "object" && "Device" in groupType) return "Device";
	return "Group";
}

export function SpaceCustomizationPanel({
	isOpen,
	onClose,
	spaceId,
}: SpaceCustomizationPanelProps) {
	const [groupType, setGroupType] = useState<GroupType>("Custom");
	const [groupName, setGroupName] = useState("");
	const [isAddingGroup, setIsAddingGroup] = useState(false);
	const addGroup = useLibraryMutation("spaces.add_group");

	if (!spaceId) return null;

	const handleAddGroup = async () => {
		if (!spaceId) return;

		try {
			const result = await addGroup.mutateAsync({
				space_id: spaceId,
				name: groupName.trim() || getDefaultGroupName(groupType),
				group_type: groupType,
			});

			// Reset form
			setGroupName("");
			setGroupType("Custom");
			setIsAddingGroup(false);

			// Scroll to the newly created group in the sidebar after a brief delay
			// (allows time for the group to be added to the DOM)
			setTimeout(() => {
				const groupElement = document.querySelector(
					`[data-group-id="${result.group.id}"]`,
				);
				if (groupElement) {
					groupElement.scrollIntoView({
						behavior: "smooth",
						block: "nearest",
					});
					// Add a temporary highlight effect
					groupElement.classList.add("ring-2", "ring-accent/50");
					setTimeout(() => {
						groupElement.classList.remove(
							"ring-2",
							"ring-accent/50",
						);
					}, 2000);
				}
			}, 100);
		} catch (err) {
			console.error("Failed to add group:", err);
		}
	};

	const content = (
		<AnimatePresence>
			{isOpen && (
				<>
					{/* Backdrop */}
					<motion.div
						initial={{ opacity: 0 }}
						animate={{ opacity: 1 }}
						exit={{ opacity: 0 }}
						transition={{ duration: 0.2 }}
						className="fixed inset-0 bg-black/20 z-[65]"
						onClick={onClose}
					/>

					{/* Panel */}
					<motion.div
						initial={{ x: -20, opacity: 0 }}
						animate={{ x: 0, opacity: 1 }}
						exit={{ x: -20, opacity: 0 }}
						transition={{ duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
						className="fixed left-[228px] top-2 bottom-2 w-[220px] z-[70]"
					>
						<div className="h-full rounded-2xl bg-sidebar flex flex-col p-2.5">
							{/* Header */}
							<div className="flex items-center justify-between px-2 py-2 mb-2">
								<div>
									<h2 className="text-sm font-semibold text-sidebar-ink">
										Customize
									</h2>
									<p className="text-xs text-sidebar-inkDull mt-0.5">
										Drag to sidebar
									</p>
								</div>
								<button
									onClick={onClose}
									className="p-1 rounded-md hover:bg-sidebar-selected/30 transition-colors"
								>
									<X size={14} className="text-sidebar-inkDull" />
								</button>
							</div>

							{/* Content */}
							<div className="flex-1 overflow-y-auto space-y-4">
								{/* Quick Access Items */}
								<div className="space-y-0.5">
									{PALETTE_ITEMS.map((item) => (
										<DraggablePaletteItem
											key={item.label}
											item={item}
										/>
									))}
								</div>

								{/* Add Group Section */}
								<div className="space-y-2 pt-2 border-t border-sidebar-line/50">
									<div className="flex items-center justify-between px-2">
										<span className="text-xs font-semibold text-sidebar-inkDull uppercase tracking-wider">
											Groups
										</span>
									</div>

									{!isAddingGroup ? (
										<button
											onClick={() => setIsAddingGroup(true)}
											className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected/30 transition-colors"
										>
											<Plus size={16} weight="bold" />
											<span>Add Group</span>
										</button>
									) : (
										<div className="space-y-2 px-2">
											<select
												value={
													typeof groupType === "string"
														? groupType
														: "Custom"
												}
												onChange={(e) =>
													setGroupType(
														e.target.value as GroupType,
													)
												}
												className="w-full rounded-md border border-sidebar-line bg-sidebar-box px-2 py-1.5 text-xs text-sidebar-ink focus:outline-none focus:ring-1 focus:ring-accent"
											>
												<option value="Devices">
													All Devices
												</option>
												<option value="Locations">
													All Locations
												</option>
												<option value="Tags">Tags</option>
												<option value="Cloud">
													Cloud Storage
												</option>
												<option value="Custom">Custom</option>
											</select>

											{groupType === "Custom" && (
												<Input
													value={groupName}
													onChange={(e) =>
														setGroupName(e.target.value)
													}
													placeholder="Group name"
													className="text-xs"
													onKeyDown={(e) => {
														if (e.key === "Enter") {
															handleAddGroup();
														} else if (e.key === "Escape") {
															setIsAddingGroup(false);
															setGroupName("");
														}
													}}
													autoFocus
												/>
											)}

											<div className="flex gap-2">
												<button
													onClick={handleAddGroup}
													className="flex-1 px-2 py-1 rounded-md bg-accent text-white text-xs font-medium hover:bg-accent/90 transition-colors"
												>
													Add
												</button>
												<button
													onClick={() => {
														setIsAddingGroup(false);
														setGroupName("");
														setGroupType("Custom");
													}}
													className="px-2 py-1 rounded-md text-xs font-medium text-sidebar-inkDull hover:bg-sidebar-selected/30 transition-colors"
												>
													Cancel
												</button>
											</div>
										</div>
									)}
								</div>
							</div>

							{/* Footer */}
							<div className="px-2 py-2 mt-2 border-t border-sidebar-line/50">
								<p className="text-xs text-sidebar-inkFaint text-center">
									Drag items to your space
								</p>
							</div>
						</div>
					</motion.div>
				</>
			)}
		</AnimatePresence>
	);

	return createPortal(content, document.body);
}

