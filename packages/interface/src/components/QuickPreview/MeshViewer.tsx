/// <reference types="@react-three/fiber" />

import { Canvas } from "@react-three/fiber";
import { OrbitControls, PerspectiveCamera } from "@react-three/drei";
import { useState, useEffect, useRef, Suspense } from "react";
import type { File } from "@sd/ts-client";
import { usePlatform } from "../../platform";
import { File as FileComponent } from "../Explorer/File";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import * as GaussianSplats3D from "@mkkellogg/gaussian-splats-3d";
import * as THREE from "three";

interface MeshViewerProps {
	file: File;
	onZoomChange?: (isZoomed: boolean) => void;
	splatUrl?: string | null; // Optional URL to Gaussian splat sidecar
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

function GaussianSplatViewer({
	url,
	onFallback,
}: {
	url: string;
	onFallback: () => void;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<any>(null);

	useEffect(() => {
		if (!containerRef.current) return;

		let cancelled = false;

		const initViewer = async () => {
			try {
				const container = containerRef.current;
				if (!container) return;

				const viewer = new GaussianSplats3D.Viewer({
					rootElement: container,
					cameraUp: [0, -1, 0],
					initialCameraPosition: [0, 0, -0.5],
					initialCameraLookAt: [0, 0, 0],
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
				});

				if (!cancelled) {
					// Try to get scene info and adjust camera
					const splatMesh = viewer.splatMesh;
					if (splatMesh) {
						console.log("[GaussianSplatViewer] SplatMesh info:", {
							splatCount: splatMesh.getSplatCount?.(),
							position: splatMesh.position,
							scale: splatMesh.scale,
						});
					}

					viewer.start();
					console.log("[GaussianSplatViewer] Viewer started");

					// Verify canvas was created
					const canvas = container.querySelector("canvas");
					if (canvas) {
						const styles = window.getComputedStyle(canvas);
						console.log("[GaussianSplatViewer] Canvas info:", {
							width: canvas.width,
							height: canvas.height,
							offsetWidth: canvas.offsetWidth,
							offsetHeight: canvas.offsetHeight,
							display: styles.display,
							visibility: styles.visibility,
							opacity: styles.opacity,
							zIndex: styles.zIndex,
							position: styles.position,
						});
					} else {
						console.error(
							"[GaussianSplatViewer] No canvas created!",
						);
					}
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
	}, [url, onFallback]);

	return (
		<div
			ref={containerRef}
			style={{
				width: "100%",
				height: "100%",
				position: "relative",
			}}
		/>
	);
}

export function MeshViewer({ file, onZoomChange, splatUrl }: MeshViewerProps) {
	const platform = usePlatform();
	const [meshUrl, setMeshUrl] = useState<string | null>(null);
	const [isGaussianSplat, setIsGaussianSplat] = useState(false);
	const [splatFailed, setSplatFailed] = useState(false);
	const [shouldLoad, setShouldLoad] = useState(false);
	const [loading, setLoading] = useState(true);

	const fileId = file.content_identity?.uuid || file.id;

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

	return (
		<div className="absolute inset-0 flex items-center justify-center bg-black">
			{isGaussianSplat && !splatFailed ? (
				<>
					<GaussianSplatViewer
						url={meshUrl}
						onFallback={() => setSplatFailed(true)}
					/>
					<div className="pointer-events-none absolute left-4 top-4 z-50 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
						Gaussian Splat
					</div>
				</>
			) : (
				<>
					<div className="pointer-events-none absolute left-4 top-4 z-10 rounded-lg bg-black/80 px-3 py-1.5 text-sm font-medium text-white backdrop-blur-xl">
						3D Mesh
					</div>
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
					<div className="pointer-events-none absolute bottom-4 left-4 z-10 rounded-lg bg-black/80 px-3 py-2 text-xs text-white/70 backdrop-blur-xl">
						<div>Left drag: Rotate</div>
						<div>Right drag: Pan</div>
						<div>Scroll: Zoom</div>
					</div>
				</>
			)}
		</div>
	);
}
