import { useState, useEffect, useRef } from "react";
import {
	useLibraryQuery,
	useCoreQuery,
	useSpacedriveClient,
} from "@sd/ts-client/hooks";
import { Interactive } from "@react-three/xr";
import * as THREE from "three";
import { colors } from "../colors";
import { getThumbnailUrl } from "../utils/sidecar";

/**
 * Native VR File Explorer
 *
 * Architecture:
 * - Left sidebar (450px): Locations list from query:locations.list
 * - Right panel: File grid from query:files.directory_listing
 * - Colors: Spacedrive dark theme from packages/ui/style/colors.scss
 * - Rendering: Canvas texture (2048x1536) on 5x3.75m plane
 *
 * VR Interactions:
 * - Point VR controller at locations in left sidebar
 * - Pull trigger to select a location
 * - UV raycasting detects exact click position on the canvas
 * - Auto-selects first location on mount
 * - Shows up to 100 files (limited for VR performance)
 *
 * TODO:
 * - Click files in grid to open/preview
 * - Navigate into subdirectories
 * - File thumbnails from sidecar system
 * - Hover effects for locations/files
 */

// Helper to extract path string from SdPath
function getLocationPath(sd_path: any): string | null {
	if (sd_path?.Physical?.path) {
		return sd_path.Physical.path;
	}
	return null;
}

export function FileExplorer() {
	const client = useSpacedriveClient();
	const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
		null,
	);
	const [hoveredLocationId, setHoveredLocationId] = useState<string | null>(
		null,
	);
	const canvasRef = useRef<HTMLCanvasElement | null>(null);
	const meshRef = useRef<THREE.Mesh>(null);

	// Thumbnail cache: URL -> HTMLImageElement
	const thumbnailCache = useRef<Map<string, HTMLImageElement>>(new Map());
	const [thumbnailsLoaded, setThumbnailsLoaded] = useState(0); // Trigger re-render when thumbnails load

	// Track library ID changes reactively
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
		client.getCurrentLibraryId(),
	);

	// First, get the list of libraries
	const { data: libraries } = useCoreQuery({
		type: "libraries.list",
		input: {
			include_stats: false,
		},
	});

	// Set the first library as the active library
	useEffect(() => {
		if (
			libraries &&
			libraries.length > 0 &&
			!client.getCurrentLibraryId()
		) {
			const firstLibrary = libraries[0];
			console.log(
				"[FileExplorer] Setting active library:",
				firstLibrary.name,
				firstLibrary.id,
			);
			client.setCurrentLibrary(firstLibrary.id);
			setCurrentLibraryId(firstLibrary.id);
		}
	}, [libraries, client]);

	// Listen for library changes
	useEffect(() => {
		const handleLibraryChange = (libraryId: string) => {
			console.log("[FileExplorer] Library changed:", libraryId);
			setCurrentLibraryId(libraryId);
		};

		client.on("library-changed", handleLibraryChange);

		return () => {
			client.off("library-changed", handleLibraryChange);
		};
	}, [client]);

	// Query locations using the typed library query hook
	const {
		data: locationsData,
		error: locationsError,
		isLoading: locationsLoading,
	} = useLibraryQuery({
		type: "locations.list",
		input: null,
	});

	const locations = locationsData?.locations ?? [];

	// Debug logging
	useEffect(() => {
		console.log("[FileExplorer] Query State:", {
			librarySet: !!currentLibraryId,
			currentLibraryId,
			librariesCount: libraries?.length,
			locationsLoading,
			locationsError: locationsError ? String(locationsError) : null,
			locationsDataExists: !!locationsData,
			locationsCount: locations.length,
			locations: locations.map((l) => ({
				id: l.id,
				name: l.name,
				sd_path: l.sd_path,
			})),
		});
	}, [
		currentLibraryId,
		libraries,
		locationsData,
		locations,
		locationsError,
		locationsLoading,
	]);

	// Find selected location to get its path
	const selectedLocation = locations.find(
		(loc) => loc.id === selectedLocationId,
	);

	// Query files for selected location
	// Note: path must be an SdPath object, not a string!
	const { data: filesData, isLoading: filesLoading } = useLibraryQuery(
		{
			type: "files.directory_listing",
			input: selectedLocation?.sd_path
				? {
						path: selectedLocation.sd_path,
						limit: 100, // Limit to 100 files for VR performance
						include_hidden: false,
						sort_by: "name",
						folders_first: true,
					}
				: null!,
		},
		{
			enabled: !!selectedLocation?.sd_path,
		},
	);

	const files = filesData?.files ?? [];

	// Load thumbnails for visible files
	useEffect(() => {
		if (!currentLibraryId || files.length === 0) return;

		// Load thumbnails for first 30 files (what we display)
		const visibleFiles = files.slice(0, 30);

		// Debug: Check if files have content identity
		const filesWithIdentity = visibleFiles.filter(
			(f) => f.content_identity?.uuid,
		);
		const filesWithThumbs = visibleFiles.filter(
			(f) =>
				f.content_identity?.uuid &&
				f.sidecars?.some((s: any) => s.kind === "thumb"),
		);

		console.log(`[FileExplorer] Thumbnail status:`, {
			totalFiles: visibleFiles.length,
			withContentIdentity: filesWithIdentity.length,
			withThumbnails: filesWithThumbs.length,
		});

		// Debug: Log first file's full structure
		if (visibleFiles.length > 0) {
			console.log("[FileExplorer] Sample file data:", {
				name: visibleFiles[0].name,
				hasContentIdentity: !!visibleFiles[0].content_identity,
				contentIdentity: visibleFiles[0].content_identity,
				hasSidecars: !!visibleFiles[0].sidecars,
				sidecarsCount: visibleFiles[0].sidecars?.length || 0,
				sidecars: visibleFiles[0].sidecars,
			});
		}

		visibleFiles.forEach((file, idx) => {
			const thumbUrl = getThumbnailUrl(file, currentLibraryId, 140);

			// Debug first file's thumbnail attempt
			if (idx === 0) {
				console.log(`[FileExplorer] First file thumb check:`, {
					name: file.name,
					hasContentId: !!file.content_identity?.uuid,
					contentUuid: file.content_identity?.uuid,
					sidecarCount: file.sidecars?.length || 0,
					thumbSidecars:
						file.sidecars?.filter((s: any) => s.kind === "thumb")
							.length || 0,
					thumbUrl,
				});
			}

			if (!thumbUrl || thumbnailCache.current.has(thumbUrl)) return;

			// Start loading thumbnail
			const img = new Image();
			img.crossOrigin = "anonymous"; // Enable CORS
			img.onload = () => {
				console.log(`[FileExplorer] ✅ Thumbnail loaded: ${file.name}`);
				thumbnailCache.current.set(thumbUrl, img);
				setThumbnailsLoaded((prev) => prev + 1); // Trigger re-render
			};
			img.onerror = () => {
				console.log(
					`[FileExplorer] ❌ Failed to load thumbnail: ${thumbUrl}`,
				);
			};
			img.src = thumbUrl;
		});
	}, [files, currentLibraryId]);

	// Render to canvas
	useEffect(() => {
		const canvas = document.createElement("canvas");
		canvas.width = 2048;
		canvas.height = 1536;
		const ctx = canvas.getContext("2d");

		if (!ctx) return;

		// Clear background
		ctx.fillStyle = colors.app;
		ctx.fillRect(0, 0, canvas.width, canvas.height);

		// === LEFT SIDEBAR: LOCATIONS ===
		const sidebarWidth = 450;

		// Sidebar background
		ctx.fillStyle = colors.sidebar;
		ctx.fillRect(0, 0, sidebarWidth, canvas.height);

		// Sidebar header
		ctx.fillStyle = colors.sidebarInk;
		ctx.font = "bold 36px sans-serif";
		ctx.fillText("Locations", 24, 50);

		// Library name (if available)
		if (libraries && libraries.length > 0) {
			ctx.font = "18px sans-serif";
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.fillText(`Library: ${libraries[0].name}`, 24, 75);
		}

		// Location count or status
		ctx.font = "20px sans-serif";
		if (!client.getCurrentLibraryId()) {
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.fillText("Initializing...", 24, 100);
		} else if (locationsLoading) {
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.fillText("Loading...", 24, 100);
		} else if (locationsError) {
			ctx.fillStyle = "#ef4444";
			ctx.fillText("Error loading locations", 24, 100);
		} else {
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.fillText(
				`${locations.length} ${locations.length === 1 ? "location" : "locations"}`,
				24,
				100,
			);
		}

		// Debug: Thumbnail stats
		if (files.length > 0) {
			const filesWithContent = files.filter(
				(f) => f.content_identity?.uuid,
			).length;
			const filesWithThumbs = files.filter(
				(f) =>
					f.content_identity?.uuid &&
					f.sidecars?.some((s: any) => s.kind === "thumb"),
			).length;

			ctx.font = "16px sans-serif";
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.fillText(
				`Debug: ${filesWithContent}/${files.length} w/ content`,
				24,
				130,
			);
			ctx.fillText(`${filesWithThumbs} w/ thumbs`, 24, 150);

			// Debug first file in detail
			if (files[0]) {
				const f = files[0];
				const thumbUrl =
					currentLibraryId &&
					getThumbnailUrl(f, currentLibraryId, 140);
				ctx.fillText(`First: ${f.name.substring(0, 15)}`, 24, 170);
				ctx.fillText(
					`  uuid: ${f.content_identity?.uuid ? "YES" : "NO"}`,
					24,
					186,
				);
				ctx.fillText(`  sidecar: ${f.sidecars?.length || 0}`, 24, 202);
				ctx.fillText(`  url: ${thumbUrl ? "YES" : "NO"}`, 24, 218);
			}
		}

		// Sidebar divider
		ctx.strokeStyle = colors.sidebarLine;
		ctx.lineWidth = 2;
		ctx.beginPath();
		ctx.moveTo(0, 115);
		ctx.lineTo(sidebarWidth, 115);
		ctx.stroke();

		// Render locations list
		let yPos = 145;

		if (locationsError) {
			// Show error
			ctx.fillStyle = "#ef4444";
			ctx.font = "24px sans-serif";
			ctx.fillText("Failed to load locations", 24, yPos);
			ctx.font = "18px sans-serif";
			ctx.fillStyle = colors.sidebarInkFaint;
			const errorMsg = String(locationsError);
			ctx.fillText(errorMsg.substring(0, 35), 24, yPos + 35);
		} else if (locationsLoading) {
			// Show loading
			ctx.fillStyle = colors.sidebarInkDull;
			ctx.font = "24px sans-serif";
			ctx.fillText("Loading locations...", 24, yPos);
		} else if (locations.length === 0) {
			// Show empty state
			ctx.fillStyle = colors.sidebarInkFaint;
			ctx.font = "24px sans-serif";
			ctx.fillText("No locations found", 24, yPos);
			ctx.font = "18px sans-serif";
			ctx.fillText("Add a location in Spacedrive", 24, yPos + 35);
		} else {
			// Render locations list
			locations.forEach((location, index) => {
				const isSelected = location.id === selectedLocationId;
				const isHovered = location.id === hoveredLocationId;
				const itemHeight = 60;

				// Item background
				if (isSelected) {
					ctx.fillStyle = colors.sidebarSelected;
					ctx.fillRect(8, yPos - 8, sidebarWidth - 16, itemHeight);
				} else if (isHovered) {
					ctx.fillStyle = colors.appHover;
					ctx.fillRect(8, yPos - 8, sidebarWidth - 16, itemHeight);
				}

				// Location icon (folder emoji as placeholder)
				ctx.font = "32px sans-serif";
				ctx.fillText("📁", 24, yPos + 26);

				// Location name
				ctx.font = "28px sans-serif";
				ctx.fillStyle = isSelected
					? colors.sidebarInk
					: colors.sidebarInkDull;
				ctx.fillText(location.name || "Unnamed", 70, yPos + 28);

				// Location path (smaller)
				ctx.font = "20px sans-serif";
				ctx.fillStyle = colors.sidebarInkFaint;
				const locationPath = getLocationPath(location.sd_path) || "";
				const pathPreview =
					locationPath.length > 35
						? "..." +
							locationPath.substring(locationPath.length - 35)
						: locationPath;
				ctx.fillText(pathPreview, 70, yPos + 50);

				yPos += itemHeight + 8;
			});
		}

		// Hint at bottom of sidebar
		ctx.fillStyle = colors.sidebarInkFaint;
		ctx.font = "18px sans-serif";
		ctx.fillText(
			"💡 Click panel to cycle locations",
			24,
			canvas.height - 30,
		);

		// === RIGHT PANEL: FILES GRID ===
		const contentX = sidebarWidth + 20;
		const contentWidth = canvas.width - sidebarWidth - 40;

		if (!selectedLocation) {
			// No location selected
			ctx.fillStyle = colors.inkFaint;
			ctx.font = "32px sans-serif";
			ctx.textAlign = "center";
			ctx.fillText(
				"← Select a location to browse files",
				sidebarWidth + contentWidth / 2,
				canvas.height / 2,
			);
		} else if (filesLoading) {
			// Loading state
			ctx.fillStyle = colors.inkDull;
			ctx.font = "32px sans-serif";
			ctx.textAlign = "center";
			ctx.fillText(
				"Loading files...",
				sidebarWidth + contentWidth / 2,
				canvas.height / 2,
			);
		} else if (files.length === 0) {
			// Empty directory
			ctx.fillStyle = colors.inkFaint;
			ctx.font = "32px sans-serif";
			ctx.textAlign = "center";
			ctx.fillText(
				"No files in this location",
				sidebarWidth + contentWidth / 2,
				canvas.height / 2,
			);
		} else {
			// Render files grid
			ctx.textAlign = "left";

			// Header
			ctx.fillStyle = colors.ink;
			ctx.font = "bold 36px sans-serif";
			ctx.fillText(selectedLocation.name, contentX, 50);

			ctx.fillStyle = colors.inkDull;
			ctx.font = "24px sans-serif";
			ctx.fillText(`${files.length} items`, contentX, 80);

			// Grid settings
			const cardWidth = 180;
			const cardHeight = 220;
			const gap = 16;
			const cols = Math.floor(contentWidth / (cardWidth + gap));
			const startY = 120;

			files.slice(0, 30).forEach((file, index) => {
				const row = Math.floor(index / cols);
				const col = index % cols;
				const x = contentX + col * (cardWidth + gap);
				const y = startY + row * (cardHeight + gap);

				// Card background
				ctx.fillStyle = colors.appBox;
				ctx.fillRect(x, y, cardWidth, cardHeight);

				// Thumbnail area
				const thumbX = x + 8;
				const thumbY = y + 8;
				const thumbWidth = cardWidth - 16;
				const thumbHeight = 140;

				const isFolder =
					file.kind === "Directory" || file.kind === "Folder";

				// Try to get loaded thumbnail
				const thumbUrl =
					currentLibraryId &&
					getThumbnailUrl(file, currentLibraryId, 140);
				const thumbImg =
					thumbUrl && thumbnailCache.current.get(thumbUrl);

				if (thumbImg) {
					// Draw actual thumbnail (fit within bounds, maintain aspect ratio)
					const aspectRatio = thumbImg.width / thumbImg.height;
					let drawWidth = thumbWidth;
					let drawHeight = thumbHeight;

					if (aspectRatio > thumbWidth / thumbHeight) {
						// Image is wider
						drawHeight = thumbWidth / aspectRatio;
					} else {
						// Image is taller
						drawWidth = thumbHeight * aspectRatio;
					}

					const drawX = thumbX + (thumbWidth - drawWidth) / 2;
					const drawY = thumbY + (thumbHeight - drawHeight) / 2;

					ctx.drawImage(
						thumbImg,
						drawX,
						drawY,
						drawWidth,
						drawHeight,
					);
				} else {
					// Fallback: colored placeholder
					ctx.fillStyle = isFolder
						? colors.appHover
						: colors.appDarkBox;
					ctx.fillRect(thumbX, thumbY, thumbWidth, thumbHeight);

					// Icon
					ctx.font = "48px sans-serif";
					ctx.textAlign = "center";
					ctx.fillText(
						isFolder ? "📁" : "📄",
						x + cardWidth / 2,
						y + 90,
					);
				}

				// File name
				ctx.font = "18px sans-serif";
				ctx.fillStyle = colors.ink;
				ctx.textAlign = "center";
				const name =
					file.name.length > 18
						? file.name.substring(0, 16) + "..."
						: file.name;
				ctx.fillText(name, x + cardWidth / 2, y + 170);

				// File kind/size
				ctx.font = "14px sans-serif";
				ctx.fillStyle = colors.inkDull;
				const kind = file.kind || "File";
				ctx.fillText(kind, x + cardWidth / 2, y + 195);
			});

			// Show "and X more" if truncated
			if (files.length > 30) {
				ctx.textAlign = "left";
				ctx.font = "24px sans-serif";
				ctx.fillStyle = colors.inkFaint;
				ctx.fillText(
					`... and ${files.length - 30} more files`,
					contentX,
					startY + Math.ceil(30 / cols) * (cardHeight + gap) + 20,
				);
			}
		}

		// Create texture
		const texture = new THREE.CanvasTexture(canvas);
		texture.needsUpdate = true;
		canvasRef.current = canvas;

		// Apply to mesh
		if (meshRef.current) {
			(meshRef.current.material as THREE.MeshBasicMaterial).map = texture;
			(meshRef.current.material as THREE.MeshBasicMaterial).needsUpdate =
				true;
		}
	}, [
		client,
		libraries,
		locations,
		selectedLocationId,
		hoveredLocationId,
		selectedLocation,
		currentLibraryId,
		files,
		filesLoading,
		locationsError,
		locationsLoading,
		thumbnailsLoaded, // Re-render when thumbnails load
	]);

	// Auto-select first location on mount
	useEffect(() => {
		if (!selectedLocationId && locations.length > 0) {
			console.log(
				"[FileExplorer] Auto-selecting first location:",
				locations[0].name,
			);
			setSelectedLocationId(locations[0].id);
		}
	}, [locations, selectedLocationId]);

	// Detect which location is at given canvas coordinates
	const getLocationAtPosition = (x: number, y: number): string | null => {
		const sidebarWidth = 450;
		const headerHeight = 115;
		const itemHeight = 68;

		if (x < sidebarWidth && y > headerHeight) {
			const clickedIndex = Math.floor((y - headerHeight) / itemHeight);
			if (clickedIndex >= 0 && clickedIndex < locations.length) {
				return locations[clickedIndex].id;
			}
		}
		return null;
	};

	// Handle VR controller select (trigger press)
	const handleSelect = (event: any) => {
		if (locations.length === 0) return;

		const uv = event.intersection?.uv;
		if (!uv) return;

		const canvasWidth = 2048;
		const canvasHeight = 1536;
		const x = uv.x * canvasWidth;
		const y = (1 - uv.y) * canvasHeight;

		console.log(
			`[FileExplorer] Click at (${Math.floor(x)}, ${Math.floor(y)})`,
		);

		const locationId = getLocationAtPosition(x, y);
		if (locationId) {
			const location = locations.find((loc) => loc.id === locationId);
			console.log("[FileExplorer] Selected location:", location?.name);
			setSelectedLocationId(locationId);
		}
	};

	// Handle VR controller hover
	const handleHover = (event: any) => {
		if (locations.length === 0) return;

		const uv = event.intersection?.uv;
		if (!uv) return;

		const canvasWidth = 2048;
		const canvasHeight = 1536;
		const x = uv.x * canvasWidth;
		const y = (1 - uv.y) * canvasHeight;

		const locationId = getLocationAtPosition(x, y);
		setHoveredLocationId(locationId);
	};

	// Handle VR controller blur
	const handleBlur = () => {
		setHoveredLocationId(null);
	};

	return (
		<Interactive
			onSelect={handleSelect}
			onHover={handleHover}
			onBlur={handleBlur}
		>
			<mesh ref={meshRef} position={[0, 1.6, -3]}>
				<planeGeometry args={[5, 3.75]} />
				<meshBasicMaterial side={THREE.DoubleSide} />
			</mesh>
		</Interactive>
	);
}
