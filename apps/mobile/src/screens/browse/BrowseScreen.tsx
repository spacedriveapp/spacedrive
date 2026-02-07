import { useState, useRef, useCallback } from "react";
import {
	View,
	Text,
	ScrollView,
	Dimensions,
	Platform,
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
import sharedColors from "@sd/ui/style/colors";
import type { SpaceItem, SpaceGroup } from "@sd/ts-client";
import { SpaceItem as SpaceItemComponent, SpaceGroupComponent } from "./components";
import { SettingsGroup } from "../../components/primitive";

interface Space {
	id: string;
	name: string;
	color: string;
}

const SCREEN_WIDTH = Dimensions.get("window").width;

function SpaceContent({
	space,
	insets
}: {
	space: Space;
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

	// Fetch space layout
	const { data: layout } = useNormalizedQuery({
		query: "spaces.get_layout",
		input: { space_id: space.id },
		resourceType: "space_layout",
		resourceId: space.id,
		enabled: !!space.id,
	});

	// Space name scale on overscroll (anchored left)
	// Note: transformOrigin doesn't work well on Android
	const isIOS = Platform.OS === 'ios';
	const spaceNameScale = useAnimatedStyle(() => {
		if (!isIOS) {
			return {};
		}
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
	const spaceItems = (layout?.space_items || []).filter(
		(item) => item.item_type !== "Overview"
	);
	const groups = layout?.groups || [];

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
						style={{ backgroundColor: space.color }}
					/>
					<Animated.Text
						style={[spaceNameScale]}
						className="text-ink text-[30px] font-bold"
					>
						{space.name}
					</Animated.Text>
				</View>
			</View>

			{/* Search Bar */}
			<View className="mb-6">
				<GlassSearchBar onPress={handleSearchPress} editable={false} />
			</View>

			{/* Space Items (pinned shortcuts) */}
			{spaceItems.length > 0 && (
				<View className="mb-6">
					<SettingsGroup>
						{spaceItems.map((item) => (
							<SpaceItemComponent key={item.id} item={item} />
						))}
					</SettingsGroup>
				</View>
			)}

			{/* Groups */}
			{groups.map(({ group, items }) => (
				<SpaceGroupComponent key={group.id} group={group} items={items} />
			))}
		</Animated.ScrollView>
	);
}

function CreateSpaceScreen() {
	return (
		<View
			style={{ width: SCREEN_WIDTH, paddingHorizontal: 16 }}
			className="flex-1 items-center justify-center"
		>
			<View className="w-16 h-16 rounded-full bg-accent/10 items-center justify-center mb-4">
				<Text className="text-accent text-3xl">+</Text>
			</View>
			<Text className="text-xl font-bold text-ink mb-2">
				Create New Space
			</Text>
			<Text className="text-ink-dull text-center max-w-xs">
				Organize your files, devices, and locations into separate spaces
			</Text>
		</View>
	);
}

export function BrowseScreen() {
	const insets = useSafeAreaInsets();
	const { data: spacesData } = useNormalizedQuery({
		query: "spaces.list",
		input: null,
		resourceType: "space",
	});
	const [currentPage, setCurrentPage] = useState(0);
	const scrollViewRef = useRef<ScrollView>(null);

	const spacesList = (spacesData?.spaces || []) as Space[];
	const totalPages = spacesList.length + 1; // +1 for create space page

	const handleScroll = useCallback(
		(event: NativeSyntheticEvent<NativeScrollEvent>) => {
			const offsetX = event.nativeEvent.contentOffset.x;
			const page = Math.round(offsetX / SCREEN_WIDTH);
			setCurrentPage(page);
		},
		[]
	);

	// Build page colors array - space colors for space pages, accent for create page
	const pageColors = [
		...spacesList.map((space) => space.color),
		`hsl(${sharedColors.accent.DEFAULT})`, // Create page uses accent color
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
				{spacesList.map((space) => (
					<SpaceContent
						key={space.id}
						space={space}
						insets={insets}
					/>
				))}
				<CreateSpaceScreen />
			</ScrollView>

			{/* Floating Space Indicator */}
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
