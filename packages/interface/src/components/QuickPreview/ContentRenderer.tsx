import type { File, ContentKind } from "@sd/ts-client";
import { File as FileComponent } from "../Explorer/File";
import { formatBytes, getContentKind } from "../Explorer/utils";
import { usePlatform } from "../../platform";
import { useServer } from "../../ServerContext";
import {
	useState,
	useEffect,
	useRef,
	useCallback,
	lazy,
	Suspense,
} from "react";
import {
	MagnifyingGlassPlus,
	MagnifyingGlassMinus,
	ArrowCounterClockwise,
	Cube,
} from "@phosphor-icons/react";
import { VideoPlayer } from "./VideoPlayer";
import type {
	VideoControlsState,
	VideoControlsCallbacks,
} from "./VideoControls";
import { AudioPlayer } from "./AudioPlayer";
import { useZoomPan } from "./useZoomPan";
import { TextViewer } from "./TextViewer";
import { WithPrismTheme } from "./prism";
import { SplatShimmerEffect } from "./SplatShimmerEffect";
import { sounds } from "@sd/assets/sounds";
import { TopBarButton } from "@sd/ui";
import { DirectoryPreview } from "./DirectoryPreview";

const MeshViewer = lazy(() =>
	import("./MeshViewer").then((m) => ({ default: m.MeshViewer })),
);
const MeshViewerUI = lazy(() =>
	import("./MeshViewer").then((m) => ({ default: m.MeshViewerUI })),
);

interface ContentRendererProps {
	file: File;
	onZoomChange?: (isZoomed: boolean) => void;
	onVideoControlsStateChange?: (state: VideoControlsState) => void;
	onShowVideoControlsChange?: (show: boolean) => void;
	getVideoCallbacks?: (callbacks: VideoControlsCallbacks) => void;
}

function ImageRenderer({ file, onZoomChange }: ContentRendererProps) {
	const platform = usePlatform();
	const { buildSidecarUrl } = useServer();
	const containerRef = useRef<HTMLDivElement>(null);
	const [originalLoaded, setOriginalLoaded] = useState(false);
	const [originalUrl, setOriginalUrl] = useState<string | null>(null);
	const [shouldLoadOriginal, setShouldLoadOriginal] = useState(false);
	const [showSplat, setShowSplat] = useState(false);
	const [splatLoaded, setSplatLoaded] = useState(false);
	const { zoom, zoomIn, zoomOut, reset, isZoomed, transform } =
		useZoomPan(containerRef);

	// Track MeshViewer controls state
	const [meshControls, setMeshControls] = useState({
		autoRotate: true,
		swayAmount: 0.25,
		swaySpeed: 0.5,
		cameraDistance: 0.5,
		isGaussianSplat: false,
	});

	// Get a stable identifier for the image file itself
	const imageFileId = file.content_identity?.uuid || file.id;

	// Check if Gaussian splat sidecar exists and get URL
	const splatSidecar = file.sidecars?.find(
		(s) => s.kind === "gaussian_splat" && s.format === "ply",
	);
	const hasSplat = !!splatSidecar;

	// Build sidecar URL for the splat
	const splatUrl =
		hasSplat && file.content_identity?.uuid
			? buildSidecarUrl(
					file.content_identity.uuid,
					splatSidecar!.kind,
					splatSidecar!.variant,
					splatSidecar!.format,
				)
			: null;

	// Notify parent of zoom state changes
	useEffect(() => {
		onZoomChange?.(isZoomed);
	}, [isZoomed, onZoomChange]);

	// Reset and defer original loading by 50ms to ensure thumbnail renders first
	useEffect(() => {
		setShouldLoadOriginal(false);
		setOriginalLoaded(false);
		setOriginalUrl(null);
		setShowSplat(false);
		setSplatLoaded(false);

		const timer = setTimeout(() => {
			setShouldLoadOriginal(true);
		}, 50);

		return () => clearTimeout(timer);
	}, [imageFileId]);

	useEffect(() => {
		if (!shouldLoadOriginal || !platform.convertFileSrc) {
			return;
		}

		const sdPath = file.sd_path as any;
		const physicalPath = sdPath?.Physical?.path;

		if (!physicalPath) {
			console.log(
				"[ImageRenderer] No physical path available, sd_path:",
				file.sd_path,
			);
			return;
		}

		const url = platform.convertFileSrc(physicalPath);
		console.log(
			"[ImageRenderer] Loading original from:",
			physicalPath,
			"-> URL:",
			url,
		);
		setOriginalUrl(url);
	}, [shouldLoadOriginal, imageFileId, file.sd_path, platform]);

	// Get highest resolution thumbnail first
	const getHighestResThumbnail = () => {
		const thumbnails =
			file.sidecars?.filter((s) => s.kind === "thumb") || [];
		if (thumbnails.length === 0) return null;

		const highest = thumbnails.sort((a, b) => {
			const aSize = parseInt(
				a.variant.split("x")[0]?.replace(/\D/g, "") || "0",
			);
			const bSize = parseInt(
				b.variant.split("x")[0]?.replace(/\D/g, "") || "0",
			);
			return bSize - aSize;
		})[0];

		const contentUuid = file.content_identity?.uuid;
		if (!contentUuid) return null;

		return buildSidecarUrl(
			contentUuid,
			highest.kind,
			highest.variant,
			highest.format,
		);
	};

	const thumbnailUrl = getHighestResThumbnail();

	// Stable callback to prevent re-renders that would reinitialize MeshViewer
	const handleSplatLoaded = useCallback(() => {
		console.log(
			"[ImageRenderer] Splat is fully visible, hiding image overlay",
		);
		setSplatLoaded(true);
		sounds.splat();
	}, []);

	// Persistent pre-mounted shimmer wrapper (stays mounted regardless of view)
	// Disabled for now
	const persistentShimmer = null;

	// Render splat view separately (not overlayed)
	if (showSplat && hasSplat && splatUrl) {
		return (
			<>
				{/* Persistent shimmer - always ready */}
				{persistentShimmer}

				{/* Fullscreen canvas layer */}
				<Suspense
					fallback={
						<div className="relative w-full h-full z-10 pointer-events-none bg-black flex items-center justify-center">
							{thumbnailUrl && (
								<img
									src={thumbnailUrl}
									alt={file.name}
									className="w-full h-full object-contain"
									draggable={false}
								/>
							)}
							{originalUrl && (
								<img
									src={originalUrl}
									alt={file.name}
									className="absolute inset-0 w-full h-full object-contain transition-opacity duration-300"
									style={{
										opacity: originalLoaded ? 1 : 0,
									}}
									draggable={false}
								/>
							)}
						</div>
					}
				>
					<MeshViewer
						file={file}
						splatUrl={splatUrl}
						onSplatLoaded={handleSplatLoaded}
						autoRotate={meshControls.autoRotate}
						swayAmount={meshControls.swayAmount}
						swaySpeed={meshControls.swaySpeed}
						cameraDistance={meshControls.cameraDistance}
						onControlsChange={setMeshControls}
					/>
				</Suspense>

				{/* Image overlay - shown during splat loading, fades out when loaded */}
				{!splatLoaded && (
					<div className="relative w-full h-full z-10 pointer-events-none bg-black flex items-center justify-center">
						{/* Thumbnail (always available) */}
						{thumbnailUrl && (
							<img
								src={thumbnailUrl}
								alt={file.name}
								className="w-full h-full object-contain"
								draggable={false}
							/>
						)}
						{/* Original image (fades in over thumbnail when ready) */}
						{originalUrl && (
							<img
								src={originalUrl}
								alt={file.name}
								className="absolute inset-0 w-full h-full object-contain transition-opacity duration-300"
								style={{ opacity: originalLoaded ? 1 : 0 }}
								draggable={false}
							/>
						)}
					</div>
				)}

				{/* Safe area UI overlay */}
				<div className="relative w-full h-full z-30 pointer-events-none">
					{/* Toggle button */}
					<div className="absolute top-4 left-4 pointer-events-auto">
						<TopBarButton
							icon={Cube}
							onClick={() => {
								setShowSplat(false);
								setSplatLoaded(false);
							}}
							title="Show Image"
							active={true}
							activeAccent={true}
						/>
					</div>

					{/* MeshViewer UI controls */}
					<Suspense fallback={null}>
						<MeshViewerUI
							autoRotate={meshControls.autoRotate}
							setAutoRotate={(v) =>
								setMeshControls((c) => ({
									...c,
									autoRotate: v,
								}))
							}
							swayAmount={meshControls.swayAmount}
							setSwayAmount={(v) =>
								setMeshControls((c) => ({
									...c,
									swayAmount: v,
								}))
							}
							swaySpeed={meshControls.swaySpeed}
							setSwaySpeed={(v) =>
								setMeshControls((c) => ({ ...c, swaySpeed: v }))
							}
							cameraDistance={meshControls.cameraDistance}
							setCameraDistance={(v) =>
								setMeshControls((c) => ({
									...c,
									cameraDistance: v,
								}))
							}
							isGaussianSplat={meshControls.isGaussianSplat}
							onResetFocalPoint={meshControls.onResetFocalPoint}
						/>
					</Suspense>
				</div>
			</>
		);
	}

	// Render image view with zoom/pan
	return (
		<div
			ref={containerRef}
			className={`relative w-full h-full flex items-center justify-center ${isZoomed ? "overflow-visible" : "overflow-hidden"}`}
		>
			{/* Persistent shimmer - always ready */}
			{persistentShimmer}

			{/* Splat Toggle (top-left) */}
			{hasSplat && (
				<div className="absolute top-4 left-4 z-10">
					<TopBarButton
						icon={Cube}
						onClick={() => {
							sounds.splatTrigger();
							setShowSplat(true);
						}}
						title="Show 3D Splat"
					/>
				</div>
			)}

			{/* Zoom Controls */}
			<div className="absolute top-4 right-4 z-10 flex flex-col gap-2">
				<button
					onClick={zoomIn}
					className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
					title="Zoom in (+)"
				>
					<MagnifyingGlassPlus size={20} weight="bold" />
				</button>
				<button
					onClick={zoomOut}
					className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
					title="Zoom out (-)"
				>
					<MagnifyingGlassMinus size={20} weight="bold" />
				</button>
				{zoom > 1 && (
					<button
						onClick={reset}
						className="rounded-lg bg-app-box/80 p-2 text-ink backdrop-blur-xl transition-colors hover:bg-app-hover"
						title="Reset zoom (0)"
					>
						<ArrowCounterClockwise size={20} weight="bold" />
					</button>
				)}
			</div>

			{/* Zoom level indicator */}
			{zoom > 1 && (
				<div className="absolute top-4 left-4 z-10 rounded-lg bg-app-box/80 px-3 py-1.5 text-sm font-medium text-ink backdrop-blur-xl">
					{Math.round(zoom * 100)}%
				</div>
			)}

			{/* Image container with zoom/pan transform */}
			<div
				className="relative w-full h-full flex items-center justify-center"
				style={transform}
			>
				{/* High-res thumbnail (always rendered as background layer) */}
				{thumbnailUrl && (
					<img
						src={thumbnailUrl}
						alt={file.name}
						className="w-full h-full object-contain"
						draggable={false}
					/>
				)}

				{/* Original image (loads async, fades in over thumbnail when ready) */}
				{originalUrl && (
					<img
						src={originalUrl}
						alt={file.name}
						className="absolute inset-0 w-full h-full object-contain"
						style={{
							opacity: originalLoaded ? 1 : 0,
							transition: "opacity 0.3s",
						}}
						onLoad={() => setOriginalLoaded(true)}
						onError={(e) =>
							console.error(
								"[ImageRenderer] Original failed to load:",
								e,
							)
						}
						draggable={false}
					/>
				)}
			</div>
		</div>
	);
}

function VideoRenderer({
	file,
	onZoomChange,
	onVideoControlsStateChange,
	onShowVideoControlsChange,
	getVideoCallbacks,
}: ContentRendererProps) {
	const platform = usePlatform();
	const [videoUrl, setVideoUrl] = useState<string | null>(null);
	const [shouldLoadVideo, setShouldLoadVideo] = useState(false);

	// Get a stable identifier for the video file itself
	const videoFileId = file.content_identity?.uuid || file.id;

	// Reset and defer video loading by 50ms to ensure thumbnail renders first
	useEffect(() => {
		setShouldLoadVideo(false);
		setVideoUrl(null);

		const timer = setTimeout(() => {
			setShouldLoadVideo(true);
		}, 50);

		return () => clearTimeout(timer);
	}, [videoFileId]);

	useEffect(() => {
		if (!shouldLoadVideo || !platform.convertFileSrc) {
			return;
		}

		const sdPath = file.sd_path as any;
		const physicalPath = sdPath?.Physical?.path;

		if (!physicalPath) {
			console.log("[VideoRenderer] No physical path available");
			return;
		}

		const url = platform.convertFileSrc(physicalPath);
		console.log(
			"[VideoRenderer] Loading video from:",
			physicalPath,
			"-> URL:",
			url,
		);
		setVideoUrl(url);
	}, [shouldLoadVideo, videoFileId, file.sd_path, platform]);

	if (!videoUrl) {
		return (
			<div className="w-full h-full flex items-center justify-center">
				<FileComponent.Thumb
					file={file}
					size={800}
					className="max-w-full max-h-full"
				/>
			</div>
		);
	}

	return (
		<VideoPlayer
			src={videoUrl}
			file={file}
			onZoomChange={onZoomChange}
			onControlsStateChange={onVideoControlsStateChange}
			onShowControlsChange={onShowVideoControlsChange}
			getCallbacks={getVideoCallbacks}
		/>
	);
}

function AudioRenderer({ file }: ContentRendererProps) {
	const platform = usePlatform();
	const [audioUrl, setAudioUrl] = useState<string | null>(null);
	const [shouldLoadAudio, setShouldLoadAudio] = useState(false);

	// Get a stable identifier for the audio file itself
	const audioFileId = file.content_identity?.uuid || file.id;

	// Reset and defer audio loading by 50ms to ensure thumbnail renders first
	useEffect(() => {
		setShouldLoadAudio(false);
		setAudioUrl(null);

		const timer = setTimeout(() => {
			setShouldLoadAudio(true);
		}, 50);

		return () => clearTimeout(timer);
	}, [audioFileId]);

	useEffect(() => {
		if (!shouldLoadAudio || !platform.convertFileSrc) {
			return;
		}

		const sdPath = file.sd_path as any;
		const physicalPath = sdPath?.Physical?.path;

		if (!physicalPath) {
			console.log("[AudioRenderer] No physical path available");
			return;
		}

		const url = platform.convertFileSrc(physicalPath);
		console.log(
			"[AudioRenderer] Loading audio from:",
			physicalPath,
			"-> URL:",
			url,
		);
		setAudioUrl(url);
	}, [shouldLoadAudio, audioFileId, file.sd_path, platform]);

	if (!audioUrl) {
		return (
			<div className="w-full h-full flex items-center justify-center">
				<div className="text-center">
					<FileComponent.Thumb file={file} size={200} />
					<div className="mt-6 text-ink text-lg font-medium">
						{file.name}
					</div>
				</div>
			</div>
		);
	}

	return <AudioPlayer src={audioUrl} file={file} />;
}

function DocumentRenderer({ file }: ContentRendererProps) {
	return (
		<div className="w-full h-full flex items-center justify-center">
			<div className="text-center">
				<FileComponent.Thumb file={file} size={200} />
				<div className="mt-6 text-ink text-lg font-medium">
					{file.name}
				</div>
				<div className="text-ink-dull text-sm mt-2 capitalize">
					{getContentKind(file) ?? "unknown"}
				</div>
				<div className="text-ink-dull text-xs mt-1">
					{formatBytes(file.size || 0)}
				</div>
			</div>
		</div>
	);
}

function TextRenderer({ file }: ContentRendererProps) {
	const platform = usePlatform();
	const [textUrl, setTextUrl] = useState<string | null>(null);
	const [shouldLoadText, setShouldLoadText] = useState(false);

	const textFileId = file.content_identity?.uuid || file.id;

	useEffect(() => {
		setShouldLoadText(false);
		setTextUrl(null);

		const timer = setTimeout(() => {
			setShouldLoadText(true);
		}, 50);

		return () => clearTimeout(timer);
	}, [textFileId]);

	useEffect(() => {
		if (!shouldLoadText || !platform.convertFileSrc) {
			return;
		}

		const sdPath = file.sd_path as any;
		const physicalPath = sdPath?.Physical?.path;

		if (!physicalPath) {
			console.log("[TextRenderer] No physical path available");
			return;
		}

		const url = platform.convertFileSrc(physicalPath);
		console.log(
			"[TextRenderer] Loading text from:",
			physicalPath,
			"-> URL:",
			url,
		);
		setTextUrl(url);
	}, [shouldLoadText, textFileId, file.sd_path, platform]);

	const extension = file.name.split(".").pop()?.toLowerCase();

	if (!textUrl) {
		return (
			<div className="w-full h-full flex items-center justify-center">
				<div className="text-center">
					<FileComponent.Thumb file={file} size={120} />
					<div className="mt-4 text-ink text-lg font-medium">
						{file.name}
					</div>
					<div className="text-ink-dull text-sm mt-2">Loading...</div>
				</div>
			</div>
		);
	}

	return (
		<>
			<WithPrismTheme />
			<TextViewer
				src={textUrl}
				codeExtension={extension}
				className="w-full h-full overflow-auto bg-app p-4 text-ink"
			/>
		</>
	);
}

function DefaultRenderer({ file }: ContentRendererProps) {
	return (
		<div className="w-full h-full flex items-center justify-center">
			<div className="text-center">
				<FileComponent.Thumb file={file} size={200} />
				<div className="mt-6 text-ink text-lg font-medium">
					{file.name}
				</div>
				<div className="text-ink-dull text-sm mt-2 capitalize">
					{getContentKind(file) ?? "unknown"}
				</div>
				<div className="text-ink-dull text-xs mt-1">
					{formatBytes(file.size || 0)}
				</div>
			</div>
		</div>
	);
}

export function ContentRenderer({
	file,
	onZoomChange,
	onVideoControlsStateChange,
	onShowVideoControlsChange,
	getVideoCallbacks,
}: ContentRendererProps) {
	// Handle directories with grid preview of subdirectories
	if (file.kind === "Directory") {
		return <DirectoryPreview file={file} />;
	}

	const kind = getContentKind(file);

	switch (kind) {
		case "image":
			return <ImageRenderer file={file} onZoomChange={onZoomChange} />;
		case "video":
			return (
				<VideoRenderer
					file={file}
					onZoomChange={onZoomChange}
					onVideoControlsStateChange={onVideoControlsStateChange}
					onShowVideoControlsChange={onShowVideoControlsChange}
					getVideoCallbacks={getVideoCallbacks}
				/>
			);
		case "audio":
			return <AudioRenderer file={file} />;
		case "mesh":
			return (
				<Suspense
					fallback={
						<div className="w-full h-full flex items-center justify-center">
							<FileComponent.Thumb file={file} size={200} />
						</div>
					}
				>
					<MeshViewer file={file} />
				</Suspense>
			);
		case "document":
		case "book":
		case "spreadsheet":
		case "presentation":
			return <DocumentRenderer file={file} />;
		case "text":
		case "code":
		case "config":
			return <TextRenderer file={file} />;
		default:
			return <DefaultRenderer file={file} />;
	}
}
