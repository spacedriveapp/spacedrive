import React, { useState } from "react";
import { View, Text, ScrollView, Pressable, Alert, Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { useCoreAction } from "../../client";
import { useAppReset } from "../../contexts";
import { useStoragePermission } from "../../hooks/useStoragePermission";
import {
	Card,
	Divider,
	Button,
	Input,
	Switch,
	SettingsGroup,
	SettingsLink,
	SettingsToggle,
	SettingsOption,
	SettingsSlider,
} from "../../components/primitive";

export function SettingsScreen() {
	const insets = useSafeAreaInsets();
	const [switchValue, setSwitchValue] = useState(false);
	const [inputValue, setInputValue] = useState("");
	const [notificationsEnabled, setNotificationsEnabled] = useState(true);
	const [darkModeEnabled, setDarkModeEnabled] = useState(false);
	const [sliderValue, setSliderValue] = useState(50);

	const resetData = useCoreAction("core.reset");
	const { resetApp } = useAppReset();
	const storagePermission = useStoragePermission();

	const handleResetData = () => {
		Alert.alert(
			"Reset All Data",
			"This will permanently delete all libraries, settings, and cached data. The app will refresh automatically. Are you sure?",
			[
				{
					text: "Cancel",
					style: "cancel",
				},
				{
					text: "Reset",
					style: "destructive",
					onPress: async () => {
						resetData.mutate(
							{ confirm: true },
							{
								onSuccess: async () => {
									// Clear AsyncStorage
									await AsyncStorage.clear();

									// Refresh the entire app
									resetApp();
								},
								onError: (error) => {
									Alert.alert(
										"Error",
										error.message || "Failed to reset data",
									);
								},
							},
						);
					},
				},
			],
		);
	};

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
				<Text className="text-2xl font-bold text-ink">
					UI Primitives Showcase
				</Text>
				<Text className="text-ink-dull text-sm mt-1">
					All available components and variants
				</Text>
			</View>

			{/* Buttons Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Buttons</Text>

				<View className="gap-2">
					<Button variant="accent" size="md" onPress={() => console.log("Accent button")}>
						Accent Button
					</Button>

					<Button variant="gray" size="md">
						Gray Button
					</Button>

					<Button variant="outline" size="md">
						Outline Button
					</Button>

					<Button variant="default" size="md">
						Default Button
					</Button>

					<Button variant="subtle" size="md">
						Subtle Button
					</Button>

					<Button variant="dotted" size="md">
						Dotted Button
					</Button>

					<View className="flex-row gap-2 items-center">
						<Button variant="accent" size="xs">
							XS
						</Button>
						<Button variant="gray" size="sm">
							Small
						</Button>
						<Button variant="outline" size="md">
							Medium
						</Button>
						<Button variant="accent" size="lg">
							Large
						</Button>
					</View>

					<Button variant="accent" size="md" disabled>
						Disabled
					</Button>
				</View>
			</View>

			{/* Cards Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Cards</Text>

				<Card className="mb-2 bg-app-box">
					<Text className="text-ink">Default Card</Text>
					<Text className="text-ink-dull text-sm mt-1">
						With subtitle text
					</Text>
				</Card>

				<Card className="mb-2 bg-accent/10 border border-accent/30">
					<Text className="text-accent font-medium">Accent Card</Text>
				</Card>

				<Card className="bg-sidebar-box">
					<Text className="text-sidebar-ink">Sidebar Card</Text>
				</Card>
			</View>

			{/* Inputs Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Inputs</Text>

				<View className="gap-3">
					<Input
						placeholder="Default input"
						value={inputValue}
						onChangeText={setInputValue}
					/>

					<Input
						variant="outline"
						placeholder="Outline variant"
						value={inputValue}
						onChangeText={setInputValue}
					/>

					<Input
						variant="filled"
						placeholder="Filled variant"
						value={inputValue}
						onChangeText={setInputValue}
					/>

					<Input
						size="sm"
						placeholder="Small size"
					/>

					<Input
						size="lg"
						placeholder="Large size"
					/>

					<Input
						placeholder="Disabled input"
						value="Cannot edit"
						disabled
					/>
				</View>
			</View>

			{/* Switch Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Switches</Text>

				<View className="space-y-3">
					<View className="flex-row items-center justify-between">
						<Text className="text-ink">Enabled Switch</Text>
						<Switch
							value={switchValue}
							onValueChange={setSwitchValue}
						/>
					</View>

					<View className="flex-row items-center justify-between">
						<Text className="text-ink">Always On</Text>
						<Switch value={true} onValueChange={() => {}} />
					</View>

					<View className="flex-row items-center justify-between">
						<Text className="text-ink-dull">Always Off</Text>
						<Switch value={false} onValueChange={() => {}} />
					</View>
				</View>
			</View>

			{/* Dividers Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Dividers</Text>

				<Text className="text-ink">Section One</Text>
				<Divider />
				<Text className="text-ink">Section Two</Text>
				<Divider />
				<Text className="text-ink">Section Three</Text>
			</View>

			{/* Typography Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Typography</Text>

				<View className="space-y-2">
					<Text className="text-3xl font-bold text-ink">
						Heading 1
					</Text>
					<Text className="text-2xl font-bold text-ink">
						Heading 2
					</Text>
					<Text className="text-xl font-semibold text-ink">
						Heading 3
					</Text>
					<Text className="text-lg font-medium text-ink">
						Heading 4
					</Text>
					<Text className="text-base text-ink">Body Text</Text>
					<Text className="text-sm text-ink-dull">
						Secondary Text
					</Text>
					<Text className="text-xs text-ink-faint uppercase tracking-wider">
						Label Text
					</Text>
				</View>
			</View>

			{/* Colors Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Color System</Text>

				<View className="space-y-4">
					{/* Accent Colors */}
					<View>
						<Text className="text-ink-dull text-xs uppercase mb-2">
							Accent
						</Text>
						<View className="flex-row flex-wrap gap-4">
							<View className="items-center">
								<View
									className="bg-accent rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">DEFAULT</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-accent-faint rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">faint</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-accent-deep rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">deep</Text>
							</View>
						</View>
					</View>

					{/* Ink Colors */}
					<View>
						<Text className="text-ink-dull text-xs uppercase mb-2">
							Ink (Text)
						</Text>
						<View className="flex-row flex-wrap gap-4">
							<View className="items-center">
								<View
									className="bg-ink rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">DEFAULT</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-ink-dull rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">dull</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-ink-faint rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">faint</Text>
							</View>
						</View>
					</View>

					{/* Sidebar Colors */}
					<View>
						<Text className="text-ink-dull text-xs uppercase mb-2">
							Sidebar
						</Text>
						<View className="flex-row flex-wrap gap-4">
							<View className="items-center">
								<View
									className="bg-sidebar rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">DEFAULT</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-box rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">box</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-line rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">line</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-ink rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">ink</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-ink-dull rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">inkDull</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-ink-faint rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">inkFaint</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-divider rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">divider</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-button rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">button</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-sidebar-selected rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">selected</Text>
							</View>
						</View>
					</View>

					{/* App Colors */}
					<View>
						<Text className="text-ink-dull text-xs uppercase mb-2">
							App
						</Text>
						<View className="flex-row flex-wrap gap-4">
							<View className="items-center">
								<View
									className="bg-app rounded-lg mb-2 border border-app-line"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">DEFAULT</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-box rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">box</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-dark-box rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">darkBox</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-overlay rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">overlay</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-line rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">line</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-frame rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">frame</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-button rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">button</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-hover rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">hover</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-app-selected rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">selected</Text>
							</View>
						</View>
					</View>

					{/* Menu Colors */}
					<View>
						<Text className="text-ink-dull text-xs uppercase mb-2">
							Menu
						</Text>
						<View className="flex-row flex-wrap gap-4">
							<View className="items-center">
								<View
									className="bg-menu rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">DEFAULT</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-line rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">line</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-hover rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">hover</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-selected rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">selected</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-shade rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">shade</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-ink rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">ink</Text>
							</View>
							<View className="items-center">
								<View
									className="bg-menu-faint rounded-lg mb-2"
									style={{ width: 80, height: 80 }}
								/>
								<Text className="text-ink-faint text-[10px]">faint</Text>
							</View>
						</View>
					</View>
				</View>
			</View>

			{/* Spacing Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">Spacing Scale</Text>

				<View className="space-y-2">
					<View className="flex-row items-center gap-3">
						<View className="w-1 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">4px (1)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-2 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">8px (2)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-3 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">12px (3)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-4 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">16px (4)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-6 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">24px (6)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-8 h-4 bg-accent" />
						<Text className="text-ink-dull text-sm">32px (8)</Text>
					</View>
				</View>
			</View>

			{/* Border Radius Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">
					Border Radius
				</Text>

				<View className="space-y-3">
					<View className="flex-row items-center gap-3">
						<View className="w-12 h-12 bg-accent rounded-sm" />
						<Text className="text-ink">Small (2px)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-12 h-12 bg-accent rounded-md" />
						<Text className="text-ink">Medium (6px)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-12 h-12 bg-accent rounded-lg" />
						<Text className="text-ink">Large (8px)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-12 h-12 bg-accent rounded-xl" />
						<Text className="text-ink">XL (12px)</Text>
					</View>
					<View className="flex-row items-center gap-3">
						<View className="w-12 h-12 bg-accent rounded-full" />
						<Text className="text-ink">Full (9999px)</Text>
					</View>
				</View>
			</View>

			{/* Interactive Demo */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">
					Interactive Demo
				</Text>

				<View className="space-y-3">
					<Pressable className="p-4 bg-app-box rounded-lg active:bg-app-hover">
						<Text className="text-ink">Pressable Card</Text>
						<Text className="text-ink-dull text-sm">
							Tap to see active state
						</Text>
					</Pressable>

					<Pressable className="p-4 bg-accent rounded-lg active:bg-accent-deep">
						<Text className="text-white font-medium">
							Accent Pressable
						</Text>
					</Pressable>
				</View>
			</View>

			{/* Settings Primitives Section */}
			<View className="mb-6">
				<Text className="text-ink font-semibold mb-3">
					iOS Settings Style
				</Text>

				<SettingsGroup header="Account">
					<SettingsLink
						icon={<View className="w-6 h-6 bg-accent rounded-full" />}
						label="Profile"
						description="View and edit your profile"
						onPress={() => console.log("Profile")}
					/>
					<SettingsLink
						icon={<View className="w-6 h-6 bg-green-500 rounded-full" />}
						label="Security"
						onPress={() => console.log("Security")}
					/>
					<SettingsToggle
						icon={<View className="w-6 h-6 bg-orange-500 rounded-full" />}
						label="Notifications"
						description="Push notifications for this library"
						value={notificationsEnabled}
						onValueChange={setNotificationsEnabled}
					/>
				</SettingsGroup>

				<SettingsGroup
					header="Appearance"
					footer="Dark mode will be applied across all libraries"
				>
					<SettingsToggle
						icon={<View className="w-6 h-6 bg-purple-500 rounded-full" />}
						label="Dark Mode"
						value={darkModeEnabled}
						onValueChange={setDarkModeEnabled}
					/>
					<SettingsOption
						icon={<View className="w-6 h-6 bg-blue-500 rounded-full" />}
						label="Theme"
						value="System"
						onPress={() => console.log("Theme picker")}
					/>
				</SettingsGroup>

				<SettingsGroup header="Storage">
					<SettingsSlider
						icon={<View className="w-6 h-6 bg-red-500 rounded-full" />}
						label="Cache Size"
						description="Maximum cache size in GB"
						value={sliderValue}
						minimumValue={10}
						maximumValue={100}
						onValueChange={setSliderValue}
					/>
					<SettingsLink
						icon={<View className="w-6 h-6 bg-yellow-500 rounded-full" />}
						label="Clear Cache"
						onPress={() => console.log("Clear cache")}
					/>
					<SettingsLink
						icon={<View className="w-6 h-6 bg-red-600 rounded-full" />}
						label="Reset All Data"
						description="Permanently delete all libraries and settings"
						onPress={handleResetData}
					/>
				</SettingsGroup>

				{/* Permissions Section (Android only) */}
				{Platform.OS === "android" && storagePermission.isRequired && (
					<SettingsGroup
						header="Permissions"
						footer="All Files Access is required to browse and index files on your device."
					>
						<SettingsLink
							icon={
								<View
									className={`w-6 h-6 rounded-full ${
										storagePermission.isGranted ? "bg-green-500" : "bg-amber-500"
									}`}
								/>
							}
							label="All Files Access"
							description={
								storagePermission.isGranted
									? "Permission granted"
									: "Tap to grant permission"
							}
							onPress={storagePermission.openSettings}
						/>
					</SettingsGroup>
				)}
			</View>

			{/* Footer */}
			<View className="items-center py-6">
				<Text className="text-ink-faint text-sm">
					Spacedrive Mobile v2
				</Text>
				<Text className="text-ink-faint text-xs mt-1">
					UI Primitives Showcase
				</Text>
			</View>
		</ScrollView>
	);
}
