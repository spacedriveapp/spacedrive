/// <reference types="@react-three/fiber" />

import { Canvas } from "@react-three/fiber";
import { OrbitControls, PerspectiveCamera } from "@react-three/drei";
import { useState, useEffect, useRef, Suspense, useCallback } from "react";
import type { File } from "@sd/ts-client";
import { usePlatform } from "../../platform";
import { File as FileComponent } from "../Explorer/File";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import * as GaussianSplats3D from "@mkkellogg/gaussian-splats-3d";
import * as THREE from "three";
import { TopBarButton, TopBarButtonGroup } from "@sd/ui";
import { Play, Pause } from "@phosphor-icons/react";

interface MeshViewerProps {
	file: File;
	onZoomChange?: (isZoomed: boolean) => void;
	splatUrl?: string | null; // Optional URL to Gaussian splat sidecar
	onSplatLoaded?: () => void; // Callback when Gaussian splat finishes loading
	// Control values (controlled component)
	autoRotate?: boolean;
	swayAmount?: number;
	swaySpeed?: number;
	cameraDistance?: number;
	onControlsChange?: (controls: {
		autoRotate: boolean;
		swayAmount: number;
		swaySpeed: number;
		cameraDistance: number;
		isGaussianSplat: boolean;
	}) => void;
}

interface MeshSceneProps {
	url: string;
}

function MeshScene({ url }: MeshSceneProps) {
	const meshRef = useRef<THREE.Mesh>(null);
	const [geometry, setGeometry] = useState<THREE.BufferGeometry | null>(null);

	useEffect(() => {
		const loader = new PLYLoader();
		loader.load(
			url,
			(loadedGeometry) => {
				loadedGeometry.computeVertexNormals();
				loadedGeometry.center();
				setGeometry(loadedGeometry);
			},
			undefined,
			(error) => {
				console.error("[MeshScene] PLY load error:", error);
			},
		);

		return () => {
			if (geometry) {
				geometry.dispose();
			}
		};
	}, [url]);

	if (!geometry) {
		return null;
	}

	return (
		// @ts-expect-error - React Three Fiber JSX types
		<mesh ref={meshRef} geometry={geometry}>
			{/* @ts-expect-error - React Three Fiber JSX types */}
			<meshStandardMaterial
				color="#888888"
				metalness={0.3}
				roughness={0.7}
			/>
			{/* @ts-expect-error - React Three Fiber JSX types */}
		</mesh>
	);
}

const CAMERA_LOOK_AT = [-0.00697, -0.00533, -0.61858] as const;

function GaussianSplatViewer({
	url,
	onFallback,
	onLoaded,
	autoRotate = false,
	swayAmount = 0.25,
	swaySpeed = 0.5,
	cameraDistance = 0.5,
}: {
	url: string;
	onFallback: () => void;
	onLoaded?: () => void;
	autoRotate?: boolean;
	swayAmount?: number;
	swaySpeed?: number;
	cameraDistance?: number;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<any>(null);
	const animationFrameRef = useRef<number | null>(null);
	const viewerReadyRef = useRef(false);
	const swayAmountRef = useRef(swayAmount);
	const swaySpeedRef = useRef(swaySpeed);
	const cameraDistanceRef = useRef(cameraDistance);

	// Update refs when props change (doesn't restart animation)
	useEffect(() => {
		swayAmountRef.current = swayAmount;
		swaySpeedRef.current = swaySpeed;
		cameraDistanceRef.current = cameraDistance;
	}, [swayAmount, swaySpeed, cameraDistance]);

	useEffect(() => {
		if (!containerRef.current) return;

		let cancelled = false;
		viewerReadyRef.current = false;

		const initViewer = async () => {
			try {
				const container = containerRef.current;
				if (!container) return;

				const viewer = new GaussianSplats3D.Viewer({
					rootElement: container,
					cameraUp: [0, -1, 0],
					initialCameraPosition: [0, 0, -0.5],
					initialCameraLookAt: [...CAMERA_LOOK_AT],
					selfDrivenMode: true,
					sphericalHarmonicsDegree: 2,
					sharedMemoryForWorkers: false,
				});

				viewerRef.current = viewer;

				await viewer.addSplatScene(url, {
					format: GaussianSplats3D.SceneFormat.Ply,
					showLoadingUI: false,
					progressiveLoad: true,
					splatAlphaRemovalThreshold: 5,
					onProgress: (percent, label, status) => {
						console.log(
							`[GaussianSplatViewer] Load progress: ${percent}% - ${label}`,
						);
					},
				});

				if (!cancelled) {
					viewer.start();
					console.log("[GaussianSplatViewer] Viewer started");

					// Set the orbit controls target to the splat's actual center
					const splatMesh = viewer.splatMesh;
					if (splatMesh && splatMesh.calculatedSceneCenter && viewer.controls) {
						viewer.controls.target.copy(splatMesh.calculatedSceneCenter);
						viewer.controls.update();
						console.log("[GaussianSplatViewer] Set focal point to splat center:", {
							x: splatMesh.calculatedSceneCenter.x,
							y: splatMesh.calculatedSceneCenter.y,
							z: splatMesh.calculatedSceneCenter.z,
						});
					}

					// Promise resolution means splat is loaded and rendering has begun
					// Call onLoaded immediately so overlay fades out as splat fades in
					viewerReadyRef.current = true;
					onLoaded?.();
				}
			} catch (err) {
				if (
					!cancelled &&
					err instanceof Error &&
					err.name !== "AbortError"
				) {
					console.error("[GaussianSplatViewer] Error:", err);
					onFallback();
				}
			}
		};

		initViewer();

		return () => {
			cancelled = true;
			if (viewerRef.current) {
				viewerRef.current.dispose();
				viewerRef.current = null;
			}
		};
	}, [url, onFallback, onLoaded]);

	// Separate effect for managing camera animation
	useEffect(() => {
		if (!autoRotate) {
			// Stop animation if it's running
			if (animationFrameRef.current) {
				cancelAnimationFrame(animationFrameRef.current);
				animationFrameRef.current = null;
			}
			return;
		}

		// Wait for viewer to be ready, then start animation
		const startAnimation = () => {
			if (!viewerReadyRef.current || !viewerRef.current) {
				// Not ready yet, check again soon
				setTimeout(startAnimation, 100);
				return;
			}

			const viewer = viewerRef.current;
			const camera = viewer.camera;
			const controls = viewer.controls;
			const startTime = Date.now();

			// Get the focal point from controls (already set to splat center)
			const focalPoint = controls ? controls.target : { x: 0, y: 0, z: 0 };

			const animate = () => {
				const elapsed = (Date.now() - startTime) / 1000;
				// Back and forth sway, not continuous rotation
				// Read from refs so values update live without restarting animation
				const angle = Math.sin(elapsed * swaySpeedRef.current) * swayAmountRef.current;

				// Gentle orbit around the focal point
				camera.position.x = focalPoint.x + Math.sin(angle) * cameraDistanceRef.current;
				camera.position.z = focalPoint.z + -Math.cos(angle) * cameraDistanceRef.current;
				camera.position.y = focalPoint.y;

				camera.lookAt(focalPoint.x, focalPoint.y, focalPoint.z);

				animationFrameRef.current = requestAnimationFrame(animate);
			};

			// Set initial camera position relative to focal point, then start animation
			requestAnimationFrame(() => {
				camera.position.set(
					focalPoint.x,
					focalPoint.y,
					focalPoint.z - cameraDistanceRef.current
				);
				camera.lookAt(focalPoint.x, focalPoint.y, focalPoint.z);
				camera.updateProjectionMatrix();

				animate();
			});
		};

		startAnimation();

		return () => {
			if (animationFrameRef.current) {
				cancelAnimationFrame(animationFrameRef.current);
				animationFrameRef.current = null;
			}
		};
	}, [autoRotate]);

	return (
		<div
			ref={containerRef}
			style={{
				width: "100%",
				height: "100%",
				position: "absolute",
				top: 0,
				left: 0,
				zIndex: 20,
			}}
		/>
	);
}

// Props for the UI controls component
interface MeshViewerUIProps {
	autoRotate: boolean;
	setAutoRotate: (value: boolean) => void;
	swayAmount: number;
	setSwayAmount: (value: number) => void;
	swaySpeed: number;
	setSwaySpeed: (value: number) => void;
	cameraDistance: number;
	setCameraDistance: (value: number) => void;
	isGaussianSplat: boolean;
}

// Export UI controls as a separate component
export function MeshViewerUI({
	autoRotate,
	setAutoRotate,
	swayAmount,
	setSwayAmount,
	swaySpeed,
	setSwaySpeed,
	cameraDistance,
	setCameraDistance,
	isGaussianSplat,
}: MeshViewerUIProps) {
	if (!isGaussianSplat) {
		return (
			<>
				<div className="pointer-events-none absolute left-4 top-4 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
					3D Mesh
				</div>
				<div className="pointer-events-none absolute bottom-4 left-4 rounded-lg bg-black/80 px-3 py-2 text-xs text-white/70 backdrop-blur-xl">
					<div>Left drag: Rotate</div>
					<div>Right drag: Pan</div>
					<div>Scroll: Zoom</div>
				</div>
			</>
		);
	}

	return (
		<>
			<div className="pointer-events-none absolute left-4 top-4 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
				Gaussian Splat
			</div>

			{/* Controls panel */}
			<div className="pointer-events-auto absolute bottom-4 right-4 flex flex-col gap-2 bg-app-box/95 backdrop-blur-lg border border-app-line rounded-lg p-3 shadow-lg min-w-[240px]">
				{/* Auto-rotate toggle */}
				<div className="flex items-center justify-between gap-2">
					<span className="text-xs text-ink font-medium">Auto Rotate</span>
					<TopBarButton
						icon={autoRotate ? Pause : Play}
						onClick={() => setAutoRotate(!autoRotate)}
						title={autoRotate ? "Pause" : "Play"}
					/>
				</div>

				{/* Sway amount slider */}
				<div className="flex flex-col gap-1">
					<div className="flex items-center justify-between">
						<label className="text-xs text-ink-dull">Sway Amount</label>
						<span className="text-xs text-ink-dull font-mono">{swayAmount.toFixed(2)}</span>
					</div>
					<input
						type="range"
						min="0"
						max="0.5"
						step="0.01"
						value={swayAmount}
						onChange={(e) => setSwayAmount(parseFloat(e.target.value))}
						className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
					/>
				</div>

				{/* Speed slider */}
				<div className="flex flex-col gap-1">
					<div className="flex items-center justify-between">
						<label className="text-xs text-ink-dull">Speed</label>
						<span className="text-xs text-ink-dull font-mono">{swaySpeed.toFixed(2)}</span>
					</div>
					<input
						type="range"
						min="0.1"
						max="2"
						step="0.1"
						value={swaySpeed}
						onChange={(e) => setSwaySpeed(parseFloat(e.target.value))}
						className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
					/>
				</div>

				{/* Distance slider */}
				<div className="flex flex-col gap-1">
					<div className="flex items-center justify-between">
						<label className="text-xs text-ink-dull">Distance</label>
						<span className="text-xs text-ink-dull font-mono">{cameraDistance.toFixed(2)}</span>
					</div>
					<input
						type="range"
						min="0.2"
						max="1.5"
						step="0.05"
						value={cameraDistance}
						onChange={(e) => setCameraDistance(parseFloat(e.target.value))}
						className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
					/>
				</div>
			</div>
		</>
	);
}

export function MeshViewer({
	file,
	onZoomChange,
	splatUrl,
	onSplatLoaded,
	autoRotate: autoRotateProp = true,
	swayAmount: swayAmountProp = 0.25,
	swaySpeed: swaySpeedProp = 0.5,
	cameraDistance: cameraDistanceProp = 0.5,
	onControlsChange,
}: MeshViewerProps) {
	const platform = usePlatform();
	const [meshUrl, setMeshUrl] = useState<string | null>(null);
	const [isGaussianSplat, setIsGaussianSplat] = useState(false);
	const [splatFailed, setSplatFailed] = useState(false);
	const [shouldLoad, setShouldLoad] = useState(false);
	const [loading, setLoading] = useState(true);

	// Use props for control values
	const autoRotate = autoRotateProp;
	const swayAmount = swayAmountProp;
	const swaySpeed = swaySpeedProp;
	const cameraDistance = cameraDistanceProp;

	// Notify parent when isGaussianSplat changes
	useEffect(() => {
		onControlsChange?.({
			autoRotate,
			swayAmount,
			swaySpeed,
			cameraDistance,
			isGaussianSplat,
		});
	}, [isGaussianSplat, autoRotate, swayAmount, swaySpeed, cameraDistance, onControlsChange]);

	const fileId = file.content_identity?.uuid || file.id;

	const handleSplatFallback = useCallback(() => {
		setSplatFailed(true);
	}, []);

	useEffect(() => {
		setShouldLoad(false);
		setMeshUrl(null);
		setLoading(true);

		const timer = setTimeout(() => {
			setShouldLoad(true);
		}, 50);

		return () => clearTimeout(timer);
	}, [fileId, splatUrl]);

	useEffect(() => {
		// If splatUrl is provided, use it directly (it's a Gaussian splat sidecar)
		if (splatUrl) {
			setMeshUrl(splatUrl);
			setIsGaussianSplat(true);
			setLoading(false);
			return;
		}

		if (!shouldLoad || !platform.convertFileSrc) {
			return;
		}

		const sdPath = file.sd_path as any;
		const physicalPath = sdPath?.Physical?.path;

		if (!physicalPath) {
			console.log("[MeshViewer] No physical path available");
			setLoading(false);
			return;
		}

		const url = platform.convertFileSrc(physicalPath);
		setMeshUrl(url);

		// Only run detection if not using splatUrl (splatUrl is already known to be a Gaussian splat)
		if (splatUrl) {
			return;
		}

		// Create an AbortController to cancel the detection fetch if component unmounts
		const abortController = new AbortController();

		fetch(url, { signal: abortController.signal })
			.then((res) => res.arrayBuffer())
			.then((buffer) => {
				const header = new TextDecoder().decode(buffer.slice(0, 3000));

				// Gaussian Splat detection
				const hasSH =
					header.includes("f_dc_0") ||
					header.includes("sh0") ||
					header.includes("sh_0");
				const hasScale =
					header.includes("scale_0") ||
					header.includes("scale_1") ||
					header.includes("scale_2");
				const hasOpacity = header.includes("opacity");
				const hasRotation =
					header.includes("rot_0") ||
					header.includes("rot_1") ||
					header.includes("rot_2") ||
					header.includes("rot_3");

				const isGS = hasSH && (hasScale || hasOpacity || hasRotation);

				setIsGaussianSplat(isGS);
				setLoading(false);
			})
			.catch((error) => {
				// Ignore abort errors (expected when component unmounts)
				if (error.name !== "AbortError") {
					console.error(
						"[MeshViewer] Error detecting format:",
						error,
					);
				}
				setLoading(false);
			});

		return () => {
			abortController.abort();
		};
	}, [shouldLoad, fileId, file.sd_path, platform, splatUrl]);

	if (!meshUrl || loading) {
		return (
			<div className="flex h-full w-full items-center justify-center">
				<div className="text-center">
					<FileComponent.Thumb file={file} size={200} />
					{loading && (
						<div className="mt-4 text-sm text-ink-dull">
							Loading 3D model...
						</div>
					)}
				</div>
			</div>
		);
	}

	// Just render the canvas, UI will be handled by ContentRenderer
	return (
		<div className="absolute inset-0 bg-black">
			{isGaussianSplat && !splatFailed ? (
				<GaussianSplatViewer
					url={meshUrl}
					onFallback={handleSplatFallback}
					onLoaded={onSplatLoaded}
					autoRotate={autoRotate}
					swayAmount={swayAmount}
					swaySpeed={swaySpeed}
					cameraDistance={cameraDistance}
				/>
			) : (
				<Canvas style={{ width: "100%", height: "100%" }}>
					<PerspectiveCamera makeDefault position={[0, 0, 5]} />
					{/* @ts-expect-error - React Three Fiber JSX types */}
					<ambientLight intensity={0.5} />
					{/* @ts-expect-error - React Three Fiber JSX types */}
					<spotLight
						position={[10, 10, 10]}
						angle={0.3}
						penumbra={0.5}
						intensity={1}
					/>
					{/* @ts-expect-error - React Three Fiber JSX types */}
					<spotLight
						position={[-10, -10, -10]}
						angle={0.3}
						penumbra={0.5}
						intensity={0.5}
					/>
					<Suspense fallback={null}>
						<MeshScene url={meshUrl} />
					</Suspense>
					<OrbitControls
						enableDamping
						dampingFactor={0.05}
						minDistance={0.5}
						maxDistance={100}
					/>
				</Canvas>
			)}
		</div>
	);
}

