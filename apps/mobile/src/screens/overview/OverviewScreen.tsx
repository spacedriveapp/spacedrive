import React, { useState, useEffect } from "react";
import { View, Text, Pressable, ScrollView, StyleSheet } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useNavigation, DrawerActions } from "@react-navigation/native";
import Animated, {
	useSharedValue,
	useAnimatedScrollHandler,
	useAnimatedStyle,
	interpolate,
	Extrapolation,
	withTiming,
	withRepeat,
	Easing,
} from "react-native-reanimated";
import { BlurView } from "expo-blur";
import { useNormalizedQuery } from "../../client";
import type { Library } from "@sd/ts-client";
import { HeroStats, DevicePanel, ActionButtons } from "./components";
import { PairingPanel } from "../../components/PairingPanel";
import { LibrarySwitcherPanel } from "../../components/LibrarySwitcherPanel";
import { GlassButton } from "../../components/GlassButton";
import { GlassSearchBar } from "../../components/GlassSearchBar";
import { JobManagerPanel } from "../../components/JobManagerPanel";
import { useRouter } from "expo-router";
import { useSearchStore } from "../explorer/context/SearchContext";
import { CircleNotch, ListBullets } from "phosphor-react-native";
import { useJobs } from "../../hooks/useJobs";

const HEADER_INITIAL_HEIGHT = 40;
const HERO_HEIGHT = 430 + HEADER_INITIAL_HEIGHT;
const HEADER_HEIGHT = 60;
const NETWORK_HEADER_HEIGHT = 50;

export function OverviewScreen() {
	const insets = useSafeAreaInsets();
	const navigation = useNavigation();
	const router = useRouter();
	const scrollY = useSharedValue(0);
	const expandedOffsetY = useSharedValue(0);
	const [showPairing, setShowPairing] = useState(false);
	const [showLibrarySwitcher, setShowLibrarySwitcher] = useState(false);
	const { enterSearchMode } = useSearchStore();
	const { activeJobCount, hasRunningJobs } = useJobs();

	// Spinning animation for jobs icon
	const spinRotation = useSharedValue(0);

	useEffect(() => {
		if (hasRunningJobs) {
			spinRotation.value = withRepeat(
				withTiming(360, { duration: 1000, easing: Easing.linear }),
				-1, // infinite
				false // don't reverse
			);
		} else {
			spinRotation.value = withTiming(0, { duration: 200 });
		}
	}, [hasRunningJobs, spinRotation]);

	const spinStyle = useAnimatedStyle(() => ({
		transform: [{ rotate: `${spinRotation.value}deg` }],
	}));

	const handleSearchPress = () => {
		router.push("/search");
	};

	const handleJobsPress = () => {
		router.push("/jobs");
	};

	// Fetch library info with real-time statistics updates
	const {
		data: libraryInfo,
		isLoading,
		error,
	} = useNormalizedQuery<null, Library>({
		query: "libraries.info",
		input: null,
		resourceType: "library",
	});

	const openDrawer = () => {
		navigation.dispatch(DrawerActions.openDrawer());
	};

	// Entrance animation on mount
	useEffect(() => {
		expandedOffsetY.value = withTiming(HERO_HEIGHT, {
			duration: 800,
			easing: Easing.out(Easing.exp),
		});
	}, [expandedOffsetY]);

	// Scroll handler - must be defined before any early returns
	const scrollHandler = useAnimatedScrollHandler({
		onScroll: (event) => {
			scrollY.value = event.contentOffset.y;
		},
	});

	// Hero parallax - moves at half speed
	const heroAnimatedStyle = useAnimatedStyle(() => {
		const translateY = interpolate(
			scrollY.value,
			[-HERO_HEIGHT, 0, HERO_HEIGHT],
			[HERO_HEIGHT / 2, 0, -(HERO_HEIGHT / 2)],
			Extrapolation.CLAMP
		);

		const opacity = interpolate(
			scrollY.value,
			[0, HERO_HEIGHT * 0.8],
			[1, 0],
			Extrapolation.CLAMP
		);

		return {
			transform: [{ translateY }],
			opacity,
		};
	});

	// Library name scale on overscroll (anchored left)
	const libraryNameScale = useAnimatedStyle(() => {
		const scale = interpolate(
			scrollY.value,
			[-200, 0],
			[1.3, 1],
			Extrapolation.CLAMP
		);

		return {
			transform: [{ scale }],
			transformOrigin: 'left center',
		};
	});

	// Blur overlay fades in as you scroll
	const blurAnimatedStyle = useAnimatedStyle(() => {
		const opacity = interpolate(
			scrollY.value,
			[80, 170],
			[0, 1],
			Extrapolation.CLAMP
		);

		return { opacity };
	});

	// Hero clipping container - clips hero at page container's top edge
	const heroClipStyle = useAnimatedStyle(() => {
		const headerTop = insets.top + HEADER_HEIGHT;
		const pinDistance = HERO_HEIGHT - headerTop;

		// Calculate page container's visual top edge position
		// This mirrors the page container's scroll transform
		const scrollOffset = interpolate(
			scrollY.value,
			[-200, 0, pinDistance],
			[200, 0, -pinDistance],
			Extrapolation.CLAMP
		);

		// Clip height = where the page container's top edge is
		const clipHeight = HERO_HEIGHT + scrollOffset;

		return {
			height: Math.max(0, clipHeight),
		};
	});

	// Page container: visual frame only - pins below header bar
	const pageContainerAnimatedStyle = useAnimatedStyle(() => {
		const headerTop = insets.top + HEADER_HEIGHT;
		const pinDistance = HERO_HEIGHT - headerTop;

		// Transform 1: Entrance animation - slides up into view
		const entranceTranslateY = interpolate(
			expandedOffsetY.value,
			[0, HERO_HEIGHT],
			[HERO_HEIGHT, 0],
			Extrapolation.CLAMP
		);

		// Transform 2: Scroll pinning - pins exactly at header height
		// Page starts at HERO_HEIGHT (420), should stop at headerTop (120)
		// So it needs to move up by pinDistance (300)
		const scrollPinTranslateY = interpolate(
			scrollY.value,
			[-200, 0, pinDistance],
			[200, 0, -pinDistance],
			Extrapolation.CLAMP
		);

		return {
			transform: [
				{ translateY: entranceTranslateY },
				{ translateY: scrollPinTranslateY },
			],
		};
	});

	// Header bar fades in when scrolling past hero
	const headerBarAnimatedStyle = useAnimatedStyle(() => {
		const opacity = interpolate(
			scrollY.value,
			[HERO_HEIGHT * 0.5, HERO_HEIGHT * 0.7],
			[0, 1],
			Extrapolation.CLAMP
		);

		return { opacity };
	});

	// "MY NETWORK" header - moves with page container, pins when it pins
	const networkHeaderStyle = useAnimatedStyle(() => {
		const headerTop = insets.top + HEADER_HEIGHT;
		const pinDistance = HERO_HEIGHT - headerTop;

		// Entrance animation - slides up into view with page
		const entranceTranslateY = interpolate(
			expandedOffsetY.value,
			[0, HERO_HEIGHT],
			[HERO_HEIGHT, 0],
			Extrapolation.CLAMP
		);

		// Scroll pinning - same as page container
		const scrollPinTranslateY = interpolate(
			scrollY.value,
			[-200, 0, pinDistance],
			[200, 0, -pinDistance],
			Extrapolation.CLAMP
		);

		return {
			transform: [
				{ translateY: entranceTranslateY },
				{ translateY: scrollPinTranslateY },
			],
		};
	});

	// Border appears only when header is pinned
	const networkHeaderBorderStyle = useAnimatedStyle(() => {
		const headerTop = insets.top + HEADER_HEIGHT;
		const pinDistance = HERO_HEIGHT - headerTop;

		const opacity = interpolate(
			scrollY.value,
			[pinDistance - 10, pinDistance],
			[0, 1],
			Extrapolation.CLAMP
		);

		return { opacity };
	});

	if (isLoading || !libraryInfo) {
		return (
			<ScrollView
				className="flex-1 bg-app"
				contentContainerStyle={{
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
				<View className="items-center justify-center py-12">
					<Text className="text-ink-dull">
						Loading library statistics...
					</Text>
				</View>
			</ScrollView>
		);
	}

	if (error) {
		return (
			<ScrollView
				className="flex-1 bg-app"
				contentContainerStyle={{
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
				<View className="items-center justify-center py-12">
					<Text className="text-red-500 font-semibold">Error</Text>
					<Text className="text-ink-dull mt-2">{String(error)}</Text>
				</View>
			</ScrollView>
		);
	}

	const stats = libraryInfo.statistics;

	return (
		<View className="flex-1 bg-black">
			{/* Hero Clipping Container - clips hero at page container's top edge */}
			<Animated.View
				pointerEvents="box-none"
				style={[
					{
						position: "absolute",
						top: 0,
						left: 0,
						right: 0,
						zIndex: 25,
						overflow: "hidden",
					},
					heroClipStyle,
				]}
			>
				{/* Hero Content - parallax and fade inside the clip */}
				<Animated.View
					pointerEvents="box-none"
					style={[
						{
							paddingTop: insets.top + HEADER_INITIAL_HEIGHT,
						},
						heroAnimatedStyle,
					]}
				>
					<View className="px-4 pb-4 flex-row items-center gap-3">
						<Animated.Text
							style={[libraryNameScale]}
							className="text-ink text-[30px] font-bold flex-1"
						>
							{libraryInfo.name}
						</Animated.Text>
						<GlassButton
							onPress={handleJobsPress}
							icon={
								<View>
									{hasRunningJobs ? (
										<Animated.View style={spinStyle}>
											<CircleNotch
												size={22}
												color="hsl(208, 100%, 57%)"
												weight="bold"
											/>
										</Animated.View>
									) : (
										<ListBullets
											size={22}
											color="hsl(235, 10%, 55%)"
											weight="bold"
										/>
									)}
									{activeJobCount > 0 && (
										<View
											className="absolute -top-1 -right-1 bg-accent rounded-full min-w-[16px] h-[16px] items-center justify-center"
										>
											<Text className="text-white text-[10px] font-bold">
												{activeJobCount > 9 ? "9+" : activeJobCount}
											</Text>
										</View>
									)}
								</View>
							}
						/>
						<GlassButton
							icon={
								<Text className="text-ink text-2xl leading-none">⋯</Text>
							}
						/>
					</View>

					{/* Search Bar */}
					<View className="px-4 mb-4" style={{ position: "relative", zIndex: 25 }} pointerEvents="auto">
						<GlassSearchBar onPress={handleSearchPress} editable={false} />
					</View>

					{/* Wrapper to elevate HeroStats above ScrollView for touch events */}
					<View style={{ position: "relative", zIndex: 25 }} pointerEvents="auto">
						<HeroStats
							totalStorage={stats.total_capacity}
							usedStorage={stats.total_capacity - stats.available_capacity}
							totalFiles={Number(stats.total_files)}
							sourceCount={stats.source_count ?? 0}
							tagCount={stats.tag_count}
							deviceCount={stats.device_count}
							uniqueContentCount={Number(stats.unique_content_count)}
							databaseSize={Number(stats.database_size)}
							sidecarCount={Number(stats.sidecar_count ?? 0)}
							sidecarSize={Number(stats.sidecar_size ?? 0)}
						/>
					</View>
				</Animated.View>
			</Animated.View>

			{/* Blur Overlay */}
			<Animated.View
				style={[
					{
						position: "absolute",
						top: 0,
						left: 0,
						right: 0,
						height: HERO_HEIGHT,
						zIndex: 2,
					},
					blurAnimatedStyle,
				]}
				pointerEvents="none"
			>
				<View className="flex-1 bg-black/40" />
			</Animated.View>

			{/* Page Container - Visual frame only (pins below header) */}
			<Animated.View
				style={[
					{
						position: "absolute",
						top: 0,
						left: 0,
						right: 0,
						zIndex: 10,
					},
					pageContainerAnimatedStyle,
				]}
				pointerEvents="none"
			>
				<View
					className="bg-app rounded-t-[30px]"
					style={{
						marginTop: HERO_HEIGHT,
						height: 2000, // Force it to be taller than screen
					}}
				/>
			</Animated.View>

			{/* Header Bar - fades in when scrolling */}
			<Animated.View
				style={[
					{
						position: "absolute",
						top: 0,
						left: 0,
						right: 0,
						height: HEADER_HEIGHT + insets.top,
						zIndex: 100,
						overflow: "hidden",
					},
					headerBarAnimatedStyle,
				]}
				pointerEvents="box-none"
			>
				<View className="flex-1">
					<BlurView
						intensity={80}
						tint="dark"
						style={StyleSheet.absoluteFill}
					/>
					<View style={StyleSheet.absoluteFill} className="bg-black/60" />
					<View
						className="flex-1 px-8 flex-row items-center gap-3"
						style={{ paddingTop: insets.top }}
					>
						<Text className="text-ink text-xl font-bold flex-1">
							{libraryInfo.name}
						</Text>
						<GlassButton
							icon={
								<Text className="text-ink text-2xl leading-none">⋯</Text>
							}
						/>
					</View>
				</View>
			</Animated.View>

			{/* MY NETWORK Header - moves with page container, pins when it pins */}
			<Animated.View
				style={[
					{
						position: "absolute",
						top: HERO_HEIGHT,
						left: 0,
						right: 0,
						zIndex: 30,
					},
					networkHeaderStyle,
				]}
				pointerEvents="none"
			>
				<View
					style={{
						height: NETWORK_HEADER_HEIGHT,
					}}
					className="bg-app overflow-hidden rounded-t-[30px] justify-center"
				>
					<Text className="text-ink-faint text-xs font-semibold text-center">
						MY NETWORK
					</Text>
					{/* Border that appears only when pinned */}
					<Animated.View
						style={[
							{
								position: "absolute",
								bottom: 0,
								left: 0,
								right: 0,
								height: 1,
							},
							networkHeaderBorderStyle,
						]}
						className="bg-app-line/30"
					/>
				</View>
			</Animated.View>

			{/* ScrollView - content scrolls normally, independently of page container */}
			<Animated.ScrollView
				style={{ zIndex: 20 }}
				contentContainerStyle={{
					paddingTop: HERO_HEIGHT + NETWORK_HEADER_HEIGHT,
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 0,
				}}
				onScroll={scrollHandler}
				scrollEventThrottle={16}
				pointerEvents="box-none"
			>
				<View className="px-4 pt-4" pointerEvents="auto">

				{/* Device Panel */}
				<DevicePanel />

				{/* Job Manager Panel */}
				<JobManagerPanel />

				{/* Action Buttons */}
				<ActionButtons
					onPairDevice={() => setShowPairing(true)}
					onSetupSync={() => {/* TODO: Open sync setup */}}
					onAddStorage={() => {/* TODO: Open source picker */}}
				/>
				</View>
			</Animated.ScrollView>

			{/* Pairing Panel */}
			<PairingPanel
				isOpen={showPairing}
				onClose={() => setShowPairing(false)}
			/>

			{/* Library Switcher Panel */}
			<LibrarySwitcherPanel
				isOpen={showLibrarySwitcher}
				onClose={() => setShowLibrarySwitcher(false)}
			/>
		</View>
	);
}
