import React from "react";
import { View, Text, FlatList, Pressable, Image } from "react-native";
import type { File } from "@sd/ts-client";
import { getVirtualMetadata, isVirtualFile, getFileKindForIcon } from "@sd/ts-client";
import { getIcon } from "@sd/assets/util/mobile";

interface ListViewProps {
	files: File[];
	onFilePress: (file: File) => void;
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatDate(dateStr: string | null): string {
	if (!dateStr) return "";
	const date = new Date(dateStr);
	const now = new Date();
	const diffMs = now.getTime() - date.getTime();
	const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

	if (diffDays === 0) return "Today";
	if (diffDays === 1) return "Yesterday";
	if (diffDays < 7) return `${diffDays} days ago`;
	if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
	if (diffDays < 365) return `${Math.floor(diffDays / 30)} months ago`;
	return `${Math.floor(diffDays / 365)} years ago`;
}

function FileRow({ file, onPress }: { file: File; onPress: () => void }) {
	const virtualMetadata = getVirtualMetadata(file);
	const isVirtual = isVirtualFile(file);

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
			className="flex-row items-center px-4 py-3 border-b border-app-line active:bg-app-hover"
		>
			{/* Icon */}
			<Image
				source={iconSource}
				className="w-10 h-10 mr-3"
				style={{ resizeMode: "contain" }}
			/>

			{/* File info */}
			<View className="flex-1">
				<Text className="text-ink font-medium" numberOfLines={1}>
					{file.name}
				</Text>
				<View className="flex-row gap-2 mt-0.5">
					{!isVirtual && file.size > 0 && (
						<Text className="text-ink-faint text-xs">
							{formatBytes(file.size)}
						</Text>
					)}
					{!isVirtual && file.modified_at && (
						<>
							<Text className="text-ink-faint text-xs">•</Text>
							<Text className="text-ink-faint text-xs">
								{formatDate(file.modified_at)}
							</Text>
						</>
					)}
					{isVirtual && virtualMetadata?.type && (
						<Text className="text-ink-faint text-xs capitalize">
							{virtualMetadata.type}
						</Text>
					)}
				</View>
			</View>

			{/* Chevron for directories */}
			{file.kind === "Directory" && (
				<Text className="text-ink-faint ml-2">›</Text>
			)}
		</Pressable>
	);
}

export function ListView({ files, onFilePress }: ListViewProps) {
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
				<FileRow file={item} onPress={() => onFilePress(item)} />
			)}
			className="flex-1"
		/>
	);
}
