import React, { useState, useMemo } from "react";
import { View, Text, Pressable, ActivityIndicator } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useExplorerFiles } from "./hooks/useExplorerFiles";
import { ListView } from "./views/ListView";
import { GridView } from "./views/GridView";
import type { Device } from "@sd/ts-client";
import { useNormalizedQuery } from "../../client";

type ViewMode = "list" | "grid";

export function ExplorerScreen() {
	const insets = useSafeAreaInsets();
	const router = useRouter();
	const searchParams = useLocalSearchParams<{
		type?: string;
		path?: string;
		view?: string;
		id?: string;
	}>();
	const [viewMode, setViewMode] = useState<ViewMode>("list");

	// Parse params into the format expected by hooks
	const params = useMemo(() => {
		if (searchParams.type === "path" && searchParams.path) {
			return { type: "path" as const, path: searchParams.path };
		}
		if (searchParams.type === "view" && searchParams.view) {
			return {
				type: "view" as const,
				view: searchParams.view,
				id: searchParams.id,
			};
		}
		return undefined;
	}, [searchParams]);

	// Fetch files
	const { files, isLoading, source } = useExplorerFiles(params);

	// Fetch device for path display
	const { data: devices } = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
		enabled: params?.type === "path",
	});

	// Get title for header
	const title = (() => {
		if (params?.type === "view") {
			if (params.view === "devices") return "Devices";
			if (params.view === "device" && params.id) {
				const device = devices?.find((d) => d.id === params.id);
				return device?.name || "Device";
			}
		}

		if (params?.type === "path") {
			try {
				const sdPath = JSON.parse(params.path);
				if (sdPath.Physical) {
					const device = devices?.find(
						(d) => d.slug === sdPath.Physical.device_slug,
					);
					const pathParts = sdPath.Physical.path.split("/").filter(Boolean);
					if (pathParts.length === 0) {
						return device?.name || "Root";
					}
					return pathParts[pathParts.length - 1];
				}
			} catch (e) {
				console.error("[ExplorerScreen] Failed to parse path:", e);
			}
		}

		return "Explorer";
	})();

	const handleFilePress = (file: any) => {
		// If it's a directory, navigate into it
		if (file.kind === "Directory") {
			router.push({
				pathname: "/explorer",
				params: {
					type: "path",
					path: JSON.stringify(file.sd_path),
				},
			});
		}
		// TODO: Handle file preview
	};

	return (
		<View className="flex-1 bg-app">
			{/* Header */}
			<View
				className="bg-app-box border-b border-app-line"
				style={{ paddingTop: insets.top }}
			>
				<View className="flex-row items-center justify-between px-4 h-14">
					{/* Back button */}
					<Pressable
						onPress={() => router.back()}
						className="w-10 h-10 items-center justify-center -ml-2"
					>
						<Text className="text-ink text-xl">←</Text>
					</Pressable>

					{/* Title */}
					<Text className="text-ink font-semibold text-lg flex-1 text-center">
						{title}
					</Text>

					{/* View mode switcher */}
					<View className="flex-row gap-1">
						<Pressable
							onPress={() => setViewMode("list")}
							className={`w-10 h-10 items-center justify-center rounded-md ${
								viewMode === "list" ? "bg-accent/10" : ""
							}`}
						>
							<Text
								className={
									viewMode === "list" ? "text-accent" : "text-ink-dull"
								}
							>
								≡
							</Text>
						</Pressable>
						<Pressable
							onPress={() => setViewMode("grid")}
							className={`w-10 h-10 items-center justify-center rounded-md ${
								viewMode === "grid" ? "bg-accent/10" : ""
							}`}
						>
							<Text
								className={
									viewMode === "grid" ? "text-accent" : "text-ink-dull"
								}
							>
								⊞
							</Text>
						</Pressable>
					</View>
				</View>
			</View>

			{/* Content */}
			{isLoading ? (
				<View className="flex-1 items-center justify-center">
					<ActivityIndicator size="large" color="hsl(208, 100%, 57%)" />
				</View>
			) : (
				<>
					{viewMode === "list" ? (
						<ListView files={files} onFilePress={handleFilePress} />
					) : (
						<GridView files={files} onFilePress={handleFilePress} />
					)}
				</>
			)}
		</View>
	);
}
