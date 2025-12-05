import React from "react";
import { View, Text, ScrollView, Pressable } from "react-native";
import { DrawerContentComponentProps } from "@react-navigation/drawer";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useSidebarStore } from "../../stores";
import { useCoreQuery, useSpacedriveClient } from "../../client";

interface SidebarSectionProps {
	title: string;
	isCollapsed: boolean;
	onToggle: () => void;
	children: React.ReactNode;
}

function SidebarSection({
	title,
	isCollapsed,
	onToggle,
	children,
}: SidebarSectionProps) {
	return (
		<View className="mb-4">
			<Pressable
				onPress={onToggle}
				className="flex-row items-center justify-between py-2"
			>
				<Text className="text-ink-dull text-xs uppercase tracking-wide">
					{title}
				</Text>
				<Text className="text-ink-faint text-xs">
					{isCollapsed ? "▶" : "▼"}
				</Text>
			</Pressable>
			{!isCollapsed && children}
		</View>
	);
}

export function SidebarContent({ navigation }: DrawerContentComponentProps) {
	const insets = useSafeAreaInsets();
	const client = useSpacedriveClient();
	const {
		currentLibraryId,
		setCurrentLibrary: setStoreLibrary,
		isGroupCollapsed,
		toggleGroup,
	} = useSidebarStore();

	// Fetch libraries
	const { data: libraries } = useCoreQuery("libraries.list", { include_stats: false });

	// Handler that syncs library ID to both store and client
	const handleSelectLibrary = (libraryId: string) => {
		console.log("[SidebarContent] Selecting library:", libraryId);
		setStoreLibrary(libraryId);
		client.setCurrentLibrary(libraryId);
	};

	const navigateAndClose = (screen: string) => {
		navigation.navigate(screen);
		navigation.closeDrawer();
	};

	return (
		<ScrollView
			className="flex-1 bg-sidebar-box"
			contentContainerStyle={{
				paddingTop: insets.top + 16,
				paddingBottom: insets.bottom + 16,
				paddingHorizontal: 16,
			}}
		>
			{/* Logo/Title */}
			<View className="mb-6">
				<Text className="text-xl font-bold text-ink">Spacedrive</Text>
				<Text className="text-ink-faint text-sm">Mobile V2</Text>
			</View>

			{/* Libraries Section */}
			<SidebarSection
				title="Libraries"
				isCollapsed={isGroupCollapsed("libraries")}
				onToggle={() => toggleGroup("libraries")}
			>
				{libraries &&
				Array.isArray(libraries) &&
				libraries.length > 0 ? (
					libraries.map((lib: any) => (
						<Pressable
							key={lib.id}
							onPress={() => handleSelectLibrary(lib.id)}
							className={`py-2.5 px-3 rounded-md mb-1 ${
								currentLibraryId === lib.id
									? "bg-sidebar-button"
									: ""
							}`}
						>
							<Text
								className={`${
									currentLibraryId === lib.id
										? "text-ink"
										: "text-ink-dull"
								}`}
							>
								{lib.name}
							</Text>
						</Pressable>
					))
				) : (
					<Text className="text-ink-faint text-sm py-2">
						No libraries
					</Text>
				)}

				<Pressable className="py-2 px-3 rounded-md border border-dashed border-sidebar-line mt-2">
					<Text className="text-ink-faint text-sm">
						+ Create Library
					</Text>
				</Pressable>
			</SidebarSection>

			{/* Locations Section */}
			<SidebarSection
				title="Locations"
				isCollapsed={isGroupCollapsed("locations")}
				onToggle={() => toggleGroup("locations")}
			>
				<Text className="text-ink-faint text-sm py-2">
					Select a library to view locations
				</Text>
			</SidebarSection>

			{/* Tags Section */}
			<SidebarSection
				title="Tags"
				isCollapsed={isGroupCollapsed("tags")}
				onToggle={() => toggleGroup("tags")}
			>
				<Text className="text-ink-faint text-sm py-2">
					Select a library to view tags
				</Text>
			</SidebarSection>

			{/* Divider */}
			<View className="h-px bg-sidebar-line my-4" />

			{/* Quick Links */}
			<View>
				<Pressable
					onPress={() => navigateAndClose("OverviewTab")}
					className="py-2.5 px-3 rounded-md mb-1"
				>
					<Text className="text-ink-dull">Overview</Text>
				</Pressable>
				<Pressable
					onPress={() => navigateAndClose("NetworkTab")}
					className="py-2.5 px-3 rounded-md mb-1"
				>
					<Text className="text-ink-dull">Network</Text>
				</Pressable>
				<Pressable
					onPress={() => navigateAndClose("SettingsTab")}
					className="py-2.5 px-3 rounded-md mb-1"
				>
					<Text className="text-ink-dull">Settings</Text>
				</Pressable>
			</View>
		</ScrollView>
	);
}
