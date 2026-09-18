import type React from "react";
import { View, Text, Pressable } from "react-native";
import { FunnelSimple, List } from "phosphor-react-native";
import { useSearchStore } from "../screens/explorer/context/SearchContext";
import type { SearchScope } from "../screens/explorer/context/SearchContext";
import { GlassButton } from "./GlassButton";
import { GlassContextMenu } from "./GlassContextMenu";

interface ScopeButtonProps {
	active: boolean;
	onPress: () => void;
	children: React.ReactNode;
}

function ScopeButton({ active, onPress, children }: ScopeButtonProps) {
	return (
		<Pressable
			onPress={onPress}
			className={`px-3 py-1.5 rounded-md ${
				active ? "bg-accent" : "bg-app-box/30"
			}`}
		>
			<Text
				className={`text-xs font-medium ${
					active ? "text-white" : "text-ink-dull"
				}`}
			>
				{children}
			</Text>
		</Pressable>
	);
}

interface SearchToolbarProps {
	viewMode?: "list" | "grid";
	setViewMode?: (mode: "list" | "grid") => void;
}

export function SearchToolbar({ viewMode, setViewMode }: SearchToolbarProps = {}) {
	const { scope, setSearchScope } = useSearchStore();

	const handleScopeChange = (newScope: SearchScope) => {
		setSearchScope(newScope);
	};

	return (
		<View className="flex-row items-center gap-3 px-4 py-2 border-b border-app-line bg-app-box/50">
			<View className="flex-row items-center gap-2">
				<Text className="text-xs font-medium text-ink-dull">Search in:</Text>
				<View className="flex-row items-center gap-1">
					<ScopeButton
						active={scope === "folder"}
						onPress={() => handleScopeChange("folder")}
					>
						This Folder
					</ScopeButton>
					<ScopeButton
						active={scope === "library"}
						onPress={() => handleScopeChange("library")}
					>
						Library
					</ScopeButton>
				</View>
			</View>

			<View className="h-4 w-px bg-app-line" />
			<View className="flex-1" />

			{viewMode && setViewMode && (
				<GlassContextMenu
					items={[
						{
							label: "List View",
							onPress: () => setViewMode("list"),
							icon: (
								<List
									size={20}
									color={
										viewMode === "list"
											? "hsl(208, 100%, 57%)"
											: "hsl(235, 10%, 55%)"
									}
									weight="bold"
								/>
							),
							active: viewMode === "list",
						},
						{
							label: "Grid View",
							onPress: () => setViewMode("grid"),
							icon: (
								<Text
									style={{
										fontSize: 20,
										color:
											viewMode === "grid"
												? "hsl(208, 100%, 57%)"
												: "hsl(235, 10%, 55%)",
									}}
								>
									⊞
								</Text>
							),
							active: viewMode === "grid",
						},
					]}
					trigger={
						<GlassButton
							icon={
								<Text className="text-ink-dull" style={{ fontSize: 18 }}>
									⋯
								</Text>
							}
							size={32}
						/>
					}
				/>
			)}

			<GlassButton
				icon={
					<FunnelSimple size={18} color="hsl(235, 10%, 55%)" weight="bold" />
				}
				size={32}
			/>

		</View>
	);
}
