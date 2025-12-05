import React, { useState } from "react";
import { View, Text, ScrollView, Pressable } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useLibraryQuery, useCoreQuery } from "../../client";
import { Card } from "../../components/primitive";
import clsx from "clsx";

// Collapsible Group Component
interface CollapsibleGroupProps {
	title: string;
	children: React.ReactNode;
	defaultCollapsed?: boolean;
}

function CollapsibleGroup({ title, children, defaultCollapsed = false }: CollapsibleGroupProps) {
	const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);

	return (
		<View className="mb-5">
			<Pressable
				onPress={() => setIsCollapsed(!isCollapsed)}
				className="flex-row items-center mb-2 px-1"
			>
				<Text className="text-ink-faint text-xs font-semibold uppercase tracking-wider mr-2">
					{isCollapsed ? "▶" : "▼"}
				</Text>
				<Text className="text-ink-faint text-xs font-semibold uppercase tracking-wider">
					{title}
				</Text>
			</Pressable>
			{!isCollapsed && (
				<View className="space-y-1">
					{children}
				</View>
			)}
		</View>
	);
}

// Sidebar Item Component
interface SidebarItemProps {
	icon: string;
	label: string;
	onPress?: () => void;
	isActive?: boolean;
	color?: string;
}

function SidebarItem({ icon, label, onPress, isActive = false, color }: SidebarItemProps) {
	return (
		<Pressable
			onPress={onPress}
			className={clsx(
				"flex-row items-center gap-2 rounded-md px-2 py-2 transition-colors",
				isActive
					? "bg-sidebar-selected/30"
					: "active:bg-sidebar-selected/20"
			)}
		>
			{color ? (
				<View
					className="w-4 h-4 rounded-full"
					style={{ backgroundColor: color }}
				/>
			) : (
				<Text className="text-base">{icon}</Text>
			)}
			<Text
				className={clsx(
					"flex-1 text-sm font-medium",
					isActive ? "text-sidebar-ink" : "text-sidebar-inkDull"
				)}
			>
				{label}
			</Text>
		</Pressable>
	);
}

// Space Switcher Component
interface Space {
	id: string;
	name: string;
	color: string;
}

function SpaceSwitcher({ spaces, currentSpace }: { spaces: Space[] | undefined; currentSpace: Space | undefined }) {
	const [showDropdown, setShowDropdown] = useState(false);

	return (
		<View className="mb-4">
			<Pressable
				onPress={() => setShowDropdown(!showDropdown)}
				className="flex-row items-center gap-2 bg-sidebar-box border border-sidebar-line rounded-lg px-3 py-2"
			>
				<View
					className="w-2 h-2 rounded-full"
					style={{ backgroundColor: currentSpace?.color || "#666" }}
				/>
				<Text className="flex-1 text-sm font-medium text-sidebar-ink">
					{currentSpace?.name || "Select Space"}
				</Text>
				<Text className="text-sidebar-inkDull text-xs">
					{showDropdown ? "▲" : "▼"}
				</Text>
			</Pressable>

			{showDropdown && spaces && spaces.length > 0 && (
				<Card className="mt-2">
					{spaces.map((space) => (
						<Pressable
							key={space.id}
							className="flex-row items-center gap-2 py-2 px-2"
							onPress={() => setShowDropdown(false)}
						>
							<View
								className="w-2 h-2 rounded-full"
								style={{ backgroundColor: space.color }}
							/>
							<Text className="text-ink text-sm">{space.name}</Text>
						</Pressable>
					))}
				</Card>
			)}
		</View>
	);
}

export function BrowseScreen() {
	const insets = useSafeAreaInsets();

	// Fetch data using queries
	const { data: locations } = useLibraryQuery("locations.list");
	const { data: tags } = useLibraryQuery("tags.list");
	const { data: spaces } = useCoreQuery("spaces.list");

	// Mock current space (first space if available)
	const currentSpace = spaces && spaces.length > 0 ? spaces[0] : undefined;

	return (
		<ScrollView
			className="flex-1 bg-sidebar"
			contentContainerStyle={{
				paddingTop: insets.top + 16,
				paddingBottom: insets.bottom + 100,
				paddingHorizontal: 16,
			}}
		>
			{/* Header */}
			<View className="mb-6">
				<Text className="text-2xl font-bold text-ink">Browse</Text>
				<Text className="text-ink-dull text-sm mt-1">
					Your libraries and spaces
				</Text>
			</View>

			{/* Space Switcher */}
			<SpaceSwitcher spaces={spaces as Space[] | undefined} currentSpace={currentSpace as Space | undefined} />

			{/* Quick Access */}
			<CollapsibleGroup title="Quick Access">
				<SidebarItem icon="🏠" label="Overview" isActive={true} />
				<SidebarItem icon="🕒" label="Recents" />
				<SidebarItem icon="❤️" label="Favorites" />
			</CollapsibleGroup>

			{/* Locations */}
			<CollapsibleGroup title="Locations">
				{locations && Array.isArray(locations) && locations.length > 0 ? (
					locations.map((loc: any) => (
						<SidebarItem
							key={loc.id}
							icon="📁"
							label={loc.name || "Unnamed"}
						/>
					))
				) : (
					<View className="px-2 py-3">
						<Text className="text-ink-dull text-sm">
							No locations added
						</Text>
					</View>
				)}
			</CollapsibleGroup>

			{/* Devices */}
			<CollapsibleGroup title="Devices">
				<SidebarItem icon="💻" label="This Device" />
			</CollapsibleGroup>

			{/* Volumes */}
			<CollapsibleGroup title="Volumes">
				<SidebarItem icon="💾" label="Macintosh HD" />
			</CollapsibleGroup>

			{/* Tags */}
			<CollapsibleGroup title="Tags">
				{tags && Array.isArray(tags) && tags.length > 0 ? (
					tags.map((tag: any) => (
						<SidebarItem
							key={tag.id}
							icon=""
							label={tag.name || "Unnamed"}
							color={tag.color || "hsl(235, 15%, 18%)"}
						/>
					))
				) : (
					<View className="px-2 py-3">
						<Text className="text-ink-dull text-sm">
							No tags created
						</Text>
					</View>
				)}
			</CollapsibleGroup>

			{/* Bottom Section */}
			<View className="mt-6 space-y-1">
				<SidebarItem icon="🔄" label="Sync Monitor" />
				<SidebarItem icon="⚙️" label="Settings" />
			</View>
		</ScrollView>
	);
}
