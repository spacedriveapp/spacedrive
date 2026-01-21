import React from "react";
import { View, Text, FlatList, Pressable, Image, Dimensions } from "react-native";
import type { File } from "@sd/ts-client";
import { getVirtualMetadata, isVirtualFile, getFileKindForIcon } from "@sd/ts-client";
import { getIcon } from "@sd/assets/util/mobile";

interface GridViewProps {
	files: File[];
	onFilePress: (file: File) => void;
}

const SCREEN_WIDTH = Dimensions.get("window").width;
const NUM_COLUMNS = 3;
const ITEM_PADDING = 8;
const CONTAINER_PADDING = 16;
const ITEM_WIDTH =
	(SCREEN_WIDTH - CONTAINER_PADDING * 2 - ITEM_PADDING * (NUM_COLUMNS - 1)) /
	NUM_COLUMNS;

function formatBytes(bytes: number | bigint | null): string {
	if (bytes === null) return "0 B";
	const numBytes = typeof bytes === "bigint" ? Number(bytes) : bytes;
	if (numBytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(numBytes) / Math.log(k));
	return Math.round(numBytes / Math.pow(k, i)) + " " + sizes[i];
}

function FileCard({ file, onPress }: { file: File; onPress: () => void }) {
	const virtualMetadata = getVirtualMetadata(file);

	// Get icon source
	const iconSource = (() => {
		// Virtual files have custom icons
		if (virtualMetadata?.iconUrl) {
			return virtualMetadata.iconUrl;
		}

		// Use getIcon from mobile-compatible util
		const kindForIcon = getFileKindForIcon(file);
		return getIcon(
			kindForIcon,
			true, // isDark - use dark mode icons for mobile
			file.extension,
			file.kind === "Directory",
		);
	})();

	return (
		<Pressable
			onPress={onPress}
			style={{ width: ITEM_WIDTH }}
			className="mb-4 active:opacity-70"
		>
			{/* Icon container - transparent bg matching web FileCard */}
			<View className="rounded-lg p-2 items-center justify-center aspect-square mb-2">
				<Image
					source={iconSource}
					className="w-20 h-20"
					style={{ resizeMode: "contain" }}
				/>
			</View>

			{/* File name */}
			<Text className="text-ink text-xs text-center" numberOfLines={2}>
				{file.name}
			</Text>

			{/* File size */}
			{file.size > 0 && (
				<Text className="text-ink-dull text-[10px] text-center mt-0.5">
					{formatBytes(file.size)}
				</Text>
			)}
		</Pressable>
	);
}

export function GridView({ files, onFilePress }: GridViewProps) {
	if (files.length === 0) {
		return (
			<View className="flex-1 items-center justify-center">
				<Text className="text-ink-dull">No items</Text>
			</View>
		);
	}

	return (
		<FlatList
			data={files}
			keyExtractor={(item) => item.id}
			renderItem={({ item }) => (
				<FileCard file={item} onPress={() => onFilePress(item)} />
			)}
			numColumns={NUM_COLUMNS}
			columnWrapperStyle={{
				paddingHorizontal: CONTAINER_PADDING,
				justifyContent: "space-between",
			}}
			contentContainerStyle={{
				paddingTop: 16,
				paddingBottom: 32,
			}}
			className="flex-1"
		/>
	);
}
