import React, { useState } from "react";
import {
	View,
	Text,
	Modal,
	Pressable,
	ScrollView,
	TextInput,
	Alert,
	ActivityIndicator,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useCoreQuery, useCoreAction, useSpacedriveClient } from "../client";
import { useSidebarStore } from "../stores";

interface LibrarySwitcherPanelProps {
	isOpen: boolean;
	onClose: () => void;
}

export function LibrarySwitcherPanel({
	isOpen,
	onClose,
}: LibrarySwitcherPanelProps) {
	const insets = useSafeAreaInsets();
	const client = useSpacedriveClient();
	const { currentLibraryId, setCurrentLibrary: setStoreLibrary } =
		useSidebarStore();
	const [showCreateForm, setShowCreateForm] = useState(false);
	const [newLibraryName, setNewLibraryName] = useState("");

	const { data: libraries, refetch } = useCoreQuery("libraries.list", {
		include_stats: false,
	});

	const createLibrary = useCoreAction("libraries.create");
	const deleteLibrary = useCoreAction("libraries.delete");

	const handleSelectLibrary = (libraryId: string) => {
		console.log("[LibrarySwitcher] Selecting library:", libraryId);
		setStoreLibrary(libraryId);
		client.setCurrentLibrary(libraryId);
		onClose();
	};

	const handleCreateLibrary = async () => {
		if (!newLibraryName.trim()) return;

		try {
			const result = await createLibrary.mutateAsync({
				name: newLibraryName,
				path: null,
			});

			setNewLibraryName("");
			setShowCreateForm(false);
			refetch();

			// Auto-select the new library
			if (result?.id) {
				handleSelectLibrary(result.id);
			}
		} catch (error) {
			Alert.alert("Error", "Failed to create library");
		}
	};

	const handleDeleteLibrary = (libraryId: string, libraryName: string) => {
		Alert.alert(
			"Delete Library",
			`Are you sure you want to delete "${libraryName}"? This action cannot be undone.`,
			[
				{ text: "Cancel", style: "cancel" },
				{
					text: "Delete",
					style: "destructive",
					onPress: async () => {
						try {
							await deleteLibrary.mutateAsync({ id: libraryId });
							refetch();

							// If deleted library was selected, switch to first available
							if (libraryId === currentLibraryId) {
								const remaining = (libraries || []).filter(
									(lib: any) => lib.id !== libraryId
								);
								if (remaining.length > 0) {
									handleSelectLibrary(remaining[0].id);
								} else {
									setStoreLibrary(null);
									client.setCurrentLibrary(null);
								}
							}
						} catch (error) {
							Alert.alert("Error", "Failed to delete library");
						}
					},
				},
			]
		);
	};

	if (!isOpen) return null;

	return (
		<Modal
			visible={isOpen}
			animationType="slide"
			transparent
			onRequestClose={onClose}
		>
			<View className="flex-1 bg-black/50">
				<View
					className="flex-1 bg-app-box rounded-t-3xl overflow-hidden"
					style={{ marginTop: insets.top + 40 }}
				>
					{/* Header */}
					<View className="px-6 py-4 border-b border-app-line">
						<View className="flex-row items-center justify-between">
							<View>
								<Text className="text-lg font-semibold text-ink">
									Libraries
								</Text>
								<Text className="text-xs text-ink-dull mt-0.5">
									Switch or manage your libraries
								</Text>
							</View>
							<Pressable
								onPress={onClose}
								className="p-2 active:bg-app-hover rounded-lg"
							>
								<Text className="text-ink-dull text-xl">✕</Text>
							</Pressable>
						</View>
					</View>

					{/* Content */}
					<ScrollView
						className="flex-1"
						contentContainerStyle={{
							padding: 16,
							paddingBottom: insets.bottom + 16,
						}}
					>
						{/* Libraries List */}
						<View className="gap-2 mb-4">
							{libraries &&
								Array.isArray(libraries) &&
								libraries.map((lib: any) => (
									<View
										key={lib.id}
										className={`p-4 rounded-lg border ${
											currentLibraryId === lib.id
												? "bg-accent/10 border-accent/30"
												: "bg-app-darkBox border-app-line"
										}`}
									>
										<Pressable
											onPress={() =>
												handleSelectLibrary(lib.id)
											}
											className="flex-1"
										>
											<View className="flex-row items-center justify-between">
												<View className="flex-1">
													<Text
														className={`font-semibold text-base ${
															currentLibraryId ===
															lib.id
																? "text-accent"
																: "text-ink"
														}`}
													>
														{lib.name}
													</Text>
													{lib.description && (
														<Text className="text-xs text-ink-dull mt-0.5">
															{lib.description}
														</Text>
													)}
													{currentLibraryId ===
														lib.id && (
														<Text className="text-xs text-accent mt-1">
															✓ Currently active
														</Text>
													)}
												</View>
												{currentLibraryId !== lib.id && (
													<Pressable
														onPress={() =>
															handleDeleteLibrary(
																lib.id,
																lib.name
															)
														}
														className="p-2 active:bg-app-hover rounded-lg ml-2"
													>
														<Text className="text-red-500 text-lg">
															🗑
														</Text>
													</Pressable>
												)}
											</View>
										</Pressable>
									</View>
								))}
						</View>

						{/* Create Library Section */}
						{!showCreateForm ? (
							<Pressable
								onPress={() => setShowCreateForm(true)}
								className="flex-row items-center justify-center gap-2 p-4 border-2 border-dashed border-accent rounded-lg bg-accent/5 active:bg-accent/10"
							>
								<Text className="text-accent text-xl">+</Text>
								<Text className="text-accent font-medium">
									Create New Library
								</Text>
							</Pressable>
						) : (
							<View className="p-4 bg-app-darkBox border border-app-line rounded-lg gap-4">
								<View>
									<Text className="text-sm font-medium text-ink mb-2">
										Library Name
									</Text>
									<TextInput
										value={newLibraryName}
										onChangeText={setNewLibraryName}
										placeholder="My Photos"
										placeholderTextColor="hsl(235, 10%, 55%)"
										className="px-4 py-3 bg-sidebar-box border border-sidebar-line rounded-lg text-ink"
										autoFocus
									/>
								</View>

								<View className="flex-row gap-3">
									<Pressable
										onPress={() => {
											setShowCreateForm(false);
											setNewLibraryName("");
										}}
										className="flex-1 px-4 py-2.5 bg-app-box border border-app-line rounded-lg active:bg-app-hover"
									>
										<Text className="text-ink-dull font-medium text-center">
											Cancel
										</Text>
									</Pressable>
									<Pressable
										onPress={handleCreateLibrary}
										disabled={
											!newLibraryName.trim() ||
											createLibrary.isPending
										}
										className={`flex-1 flex-row items-center justify-center gap-2 px-4 py-2.5 rounded-lg ${
											!newLibraryName.trim() ||
											createLibrary.isPending
												? "bg-accent/50"
												: "bg-accent active:bg-accent/90"
										}`}
									>
										{createLibrary.isPending && (
											<ActivityIndicator
												size="small"
												color="white"
											/>
										)}
										<Text className="text-white font-medium">
											{createLibrary.isPending
												? "Creating..."
												: "Create"}
										</Text>
									</Pressable>
								</View>
							</View>
						)}
					</ScrollView>
				</View>
			</View>
		</Modal>
	);
}
