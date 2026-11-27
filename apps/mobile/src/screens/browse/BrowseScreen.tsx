import { useState, useRef, useCallback } from "react";
import {
	View,
	Text,
	ScrollView,
	Dimensions,
	type NativeScrollEvent,
	type NativeSyntheticEvent,
} from "react-native";
import { useSafeAreaInsets, type EdgeInsets } from "react-native-safe-area-context";
import Animated, {
	useSharedValue,
	useAnimatedScrollHandler,
	useAnimatedStyle,
	interpolate,
	Extrapolation,
} from "react-native-reanimated";
import { useNormalizedQuery } from "../../client";
import { PageIndicator } from "../../components/PageIndicator";
import { GlassSearchBar } from "../../components/GlassSearchBar";
import { useRouter } from "expo-router";
import type { LayoutItem, LayoutGroup } from "@sd/ts-client";
import { LayoutItem as LayoutItemComponent, LayoutGroupComponent } from "./components";
import { SettingsGroup } from "../../components/primitive";

interface Layout {
	id: string;
	name: string;
	color: string;
}

const SCREEN_WIDTH = Dimensions.get("window").width;

function LayoutContent({
	layout,
	insets
}: {
	layout: Layout;
	insets: EdgeInsets;
}) {
	const router = useRouter();
	const scrollY = useSharedValue(0);

	const scrollHandler = useAnimatedScrollHandler({
		onScroll: (event) => {
			scrollY.value = event.contentOffset.y;
		},
	});

	const handleSearchPress = () => {
		router.push("/search");
	};

	const { data: sidebar } = useNormalizedQuery({
		query: "layouts.get_sidebar",
		input: { layout_id: layout.id },
		resourceType: "sidebar_layout",
		resourceId: layout.id,
		enabled: !!layout.id,
	});

	// Layout name scale on overscroll (anchored left)
	const layoutNameScale = useAnimatedStyle(() => {
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

	// Filter out Overview items (mobile doesn't show Overview in browse tab)
	const layoutItems = (sidebar?.layout_items || []).filter(
		(item) => item.item_type !== "Overview"
	);
	const groups = sidebar?.groups || [];

	return (
		<Animated.ScrollView
			style={{ width: SCREEN_WIDTH }}
			contentContainerStyle={{
				paddingTop: insets.top + 45,
				paddingHorizontal: 16,
				paddingBottom: insets.bottom + 60,
			}}
			showsVerticalScrollIndicator={false}
			onScroll={scrollHandler}
			scrollEventThrottle={16}
		>
			{/* Header */}
			<View className="mb-6">
				<View className="flex-row items-center gap-2">
					<View
						className="w-4 h-4 mx-1 rounded-full"
						style={{ backgroundColor: layout.color }}
					/>
					<Animated.Text
						style={[layoutNameScale]}
						className="text-ink text-[30px] font-bold"
					>
						{layout.name}
					</Animated.Text>
				</View>
			</View>

			{/* Search Bar */}
			<View className="mb-6">
				<GlassSearchBar onPress={handleSearchPress} editable={false} />
			</View>

			{/* Layout Items (pinned shortcuts) */}
			{layoutItems.length > 0 && (
				<View className="mb-6">
					<SettingsGroup>
						{layoutItems.map((item) => (
							<LayoutItemComponent key={item.id} item={item} />
						))}
					</SettingsGroup>
				</View>
			)}

			{/* Groups */}
			{groups.map(({ group, items }) => (
				<LayoutGroupComponent key={group.id} group={group} items={items} />
			))}
		</Animated.ScrollView>
	);
}

function CreateLayoutScreen() {
	return (
		<View
			style={{ width: SCREEN_WIDTH, paddingHorizontal: 16 }}
			className="flex-1 items-center justify-center"
		>
			<View className="w-16 h-16 rounded-full bg-accent/10 items-center justify-center mb-4">
				<Text className="text-accent text-3xl">+</Text>
			</View>
			<Text className="text-xl font-bold text-ink mb-2">
				Create New Layout
			</Text>
			<Text className="text-ink-dull text-center max-w-xs">
				Organize your files, devices, and sources into separate layouts
			</Text>
		</View>
	);
}

export function BrowseScreen() {
	const insets = useSafeAreaInsets();
	const { data: layoutsData } = useNormalizedQuery({
		query: "layouts.list",
		input: null,
		resourceType: "layout",
	});
	const [currentPage, setCurrentPage] = useState(0);
	const scrollViewRef = useRef<ScrollView>(null);

	const layoutsList = (layoutsData?.layouts || []) as Layout[];
	const totalPages = layoutsList.length + 1; // +1 for create layout page

	const handleScroll = useCallback(
		(event: NativeSyntheticEvent<NativeScrollEvent>) => {
			const offsetX = event.nativeEvent.contentOffset.x;
			const page = Math.round(offsetX / SCREEN_WIDTH);
			setCurrentPage(page);
		},
		[]
	);

	// Build page colors array - layout colors for layout pages, accent for create page
	const pageColors = [
		...layoutsList.map((layout) => layout.color),
		"hsl(208, 100%, 57%)", // Create page uses accent color
	];

	return (
		<View className="flex-1 bg-app">
			<ScrollView
				ref={scrollViewRef}
				horizontal
				pagingEnabled
				showsHorizontalScrollIndicator={false}
				onScroll={handleScroll}
				scrollEventThrottle={16}
				decelerationRate="fast"
			>
				{layoutsList.map((layout) => (
					<LayoutContent
						key={layout.id}
						layout={layout}
						insets={insets}
					/>
				))}
				<CreateLayoutScreen />
			</ScrollView>

			{/* Floating Layout Indicator */}
			<View
				className="absolute left-0 right-0"
				style={{
					top: insets.top + 16,
				}}
				pointerEvents="none"
			>
				<View className="mb-4">
					<PageIndicator
						currentIndex={currentPage}
						totalPages={totalPages}
						pageColors={pageColors}
					/>
				</View>
			</View>
		</View>
	);
}
