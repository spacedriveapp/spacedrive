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
import {
	Play,
	Pause,
	ArrowCounterClockwise,
	Sliders,
} from "@phosphor-icons/react";

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
		onResetFocalPoint?: () => void; // Reset to initial raycast focal point
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
	onResetReady,
	onDistanceCalculated,
}: {
	url: string;
	onFallback: () => void;
	onLoaded?: () => void;
	autoRotate?: boolean;
	swayAmount?: number;
	swaySpeed?: number;
	cameraDistance?: number;
	onResetReady?: (resetFn: () => void) => void;
	onDistanceCalculated?: (distance: number) => void;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<any>(null);
	const animationFrameRef = useRef<number | null>(null);
	const viewerReadyRef = useRef(false);
	const raycastCompleteRef = useRef(false);
	const swayAmountRef = useRef(swayAmount);
	const swaySpeedRef = useRef(swaySpeed);
	const cameraDistanceRef = useRef(cameraDistance);
	const currentCameraDistanceRef = useRef(cameraDistance); // Actual interpolated distance
	const focalPointRef = useRef({ x: 0, y: 0, z: 0 });
	const targetFocalPointRef = useRef({ x: 0, y: 0, z: 0 });
	const initialRaycastFocalPointRef = useRef<{
		x: number;
		y: number;
		z: number;
	} | null>(null);
	const focalPointTransitionRef = useRef({
		active: false,
		startTime: 0,
		duration: 800,
		startFocalPoint: { x: 0, y: 0, z: 0 },
		cameraOffset: { x: 0, y: 0, z: 0 },
	});

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

					// Set controls.target to the calculated center
					const splatMesh = viewer.splatMesh;
					if (splatMesh?.calculatedSceneCenter) {
						const center = splatMesh.calculatedSceneCenter;

						// Initialize focal point refs with calculated center
						focalPointRef.current = {
							x: center.x,
							y: center.y,
							z: center.z,
						};
						targetFocalPointRef.current = {
							x: center.x,
							y: center.y,
							z: center.z,
						};

						// Update controls target first
						viewer.controls.target.copy(center);

						// Position camera to look at center
						viewer.camera.position.set(
							center.x,
							center.y,
							center.z - 0.5,
						);
						viewer.camera.lookAt(center.x, center.y, center.z);
						viewer.camera.updateProjectionMatrix();
						viewer.controls.update();

						console.log(
							"[GaussianSplatViewer] Setup for raycast:",
							{
								center: {
									x: center.x,
									y: center.y,
									z: center.z,
								},
								cameraPos: {
									x: viewer.camera.position.x,
									y: viewer.camera.position.y,
									z: viewer.camera.position.z,
								},
								controlsTarget: {
									x: viewer.controls.target.x,
									y: viewer.controls.target.y,
									z: viewer.controls.target.z,
								},
							},
						);

						// Try raycast from screen center to find actual visual focal point
						// Retry multiple times since splat mesh needs time to be ready
						const container = containerRef.current;
						if (container && viewer.raycaster) {
							let retryCount = 0;
							const maxRetries = 50;
							const retryDelay = 100;

							const attemptRaycast = () => {
								retryCount++;

								const renderDimensions = {
									x: container.offsetWidth,
									y: container.offsetHeight,
								};
								const centerPosition = {
									x: renderDimensions.x / 2,
									y: renderDimensions.y / 2,
								};

								const outHits: any[] = [];
								viewer.raycaster.setFromCameraAndScreenPosition(
									viewer.camera,
									centerPosition,
									renderDimensions,
								);
								viewer.raycaster.intersectSplatMesh(
									viewer.splatMesh,
									outHits,
								);

								if (outHits.length > 0) {
									console.log(
										`[GaussianSplatViewer] ✓ Raycast SUCCESS (attempt ${retryCount})!`,
										{
											hitCount: outHits.length,
											allHits: outHits.map(
												(h: any, i: number) => ({
													index: i,
													origin: {
														x: h.origin?.x,
														y: h.origin?.y,
														z: h.origin?.z,
													},
													distance: h.distance,
												}),
											),
											cameraPosition: {
												x: viewer.camera.position.x,
												y: viewer.camera.position.y,
												z: viewer.camera.position.z,
											},
											calculatedCenter: {
												x: viewer.splatMesh
													.calculatedSceneCenter.x,
												y: viewer.splatMesh
													.calculatedSceneCenter.y,
												z: viewer.splatMesh
													.calculatedSceneCenter.z,
											},
										},
									);

									// Use the CLOSEST hit (smallest distance)
									const closestHit = outHits.reduce(
										(closest: any, hit: any) =>
											hit.distance < closest.distance
												? hit
												: closest,
										outHits[0],
									);

									const intersectionPoint = closestHit.origin;

									console.log(
										`[GaussianSplatViewer] Using closest hit:`,
										{
											origin: {
												x: intersectionPoint.x,
												y: intersectionPoint.y,
												z: intersectionPoint.z,
											},
											distance: closestHit.distance,
										},
									);

									// Set the focal point directly - no transition needed since animation hasn't started
									focalPointRef.current = {
										x: intersectionPoint.x,
										y: intersectionPoint.y,
										z: intersectionPoint.z,
									};
									targetFocalPointRef.current = {
										...focalPointRef.current,
									};

									// Save as initial raycast focal point for reset functionality
									if (!initialRaycastFocalPointRef.current) {
										initialRaycastFocalPointRef.current = {
											...focalPointRef.current,
										};
										console.log(
											"[GaussianSplatViewer] Saved initial raycast focal point:",
											initialRaycastFocalPointRef.current,
										);

										// Provide reset function to parent
										onResetReady?.(() => {
											if (
												initialRaycastFocalPointRef.current &&
												viewerRef.current
											) {
												const viewer =
													viewerRef.current;
												const initial =
													initialRaycastFocalPointRef.current;
												const current =
													viewer.controls.target;

												// Check if we're already at the initial point (avoid unnecessary transition)
												const distance = Math.sqrt(
													Math.pow(
														current.x - initial.x,
														2,
													) +
														Math.pow(
															current.y -
																initial.y,
															2,
														) +
														Math.pow(
															current.z -
																initial.z,
															2,
														),
												);

												if (distance < 0.01) {
													console.log(
														"[GaussianSplatViewer] Already at initial focal point, skipping reset",
													);
													return;
												}

												console.log(
													"[GaussianSplatViewer] Resetting from",
													{
														x: current.x,
														y: current.y,
														z: current.z,
													},
													"to initial:",
													initial,
												);
												viewer.previousCameraTarget.copy(
													current,
												);
												viewer.nextCameraTarget.copy(
													initial,
												);
												viewer.transitioningCameraTarget = true;
												viewer.transitioningCameraTargetStartTime =
													performance.now() / 1000;
											}
										});
									}

									// Calculate ACTUAL distance from camera to new focal point
									// Use this as the orbital radius to prevent zoom
									const currentCameraPos =
										viewer.camera.position;
									const actualDistance = Math.sqrt(
										Math.pow(
											currentCameraPos.x -
												intersectionPoint.x,
											2,
										) +
											Math.pow(
												currentCameraPos.y -
													intersectionPoint.y,
												2,
											) +
											Math.pow(
												currentCameraPos.z -
													intersectionPoint.z,
												2,
											),
									);

									// Set both distance refs to the actual current distance
									cameraDistanceRef.current = actualDistance;
									currentCameraDistanceRef.current =
										actualDistance;

									// Notify parent to sync the distance slider
									onDistanceCalculated?.(actualDistance);

									console.log(
										"[GaussianSplatViewer] Calculated orbital distance:",
										actualDistance,
									);

									// Update controls target
									viewer.controls.target.copy(
										intersectionPoint,
									);
									viewer.controls.update();

									// Mark raycast as complete so animation can start
									raycastCompleteRef.current = true;

									console.log(
										`[GaussianSplatViewer] Raycast complete! Focal point:`,
										focalPointRef.current,
										"Orbital distance:",
										actualDistance,
									);
								} else if (retryCount < maxRetries) {
									// Retry
									console.log(
										`[GaussianSplatViewer] Raycast attempt ${retryCount} failed, retrying...`,
									);
									setTimeout(attemptRaycast, retryDelay);
								} else {
									console.log(
										`[GaussianSplatViewer] ✗ Raycast failed after ${maxRetries} attempts - using calculatedSceneCenter`,
									);
									// Mark as complete anyway so animation can start
									raycastCompleteRef.current = true;
								}
							};

							// Start first attempt after a short delay
							setTimeout(attemptRaycast, 200);
						}
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

		let startTimeoutId: number | null = null;

		// Wait for viewer to be ready AND raycast to complete, then start animation
		const startAnimation = () => {
			if (
				!viewerReadyRef.current ||
				!viewerRef.current ||
				!raycastCompleteRef.current
			) {
				// Not ready yet, check again soon
				startTimeoutId = setTimeout(
					startAnimation,
					100,
				) as unknown as number;
				return;
			}

			const viewer = viewerRef.current;
			const camera = viewer.camera;
			const controls = viewer.controls;
			const startTime = Date.now();

			// Clear any accumulated damping/panning that might interfere with our animation
			if (controls) {
				controls.clearDampedRotation();
				controls.clearDampedPan();
				// Save current state as the "home" position
				controls.saveState();
			}

			// DEBUG: Log everything about the controls state
			console.log("[Animation Start] Controls state:", {
				target: controls
					? {
							x: controls.target.x,
							y: controls.target.y,
							z: controls.target.z,
						}
					: null,
				cameraPosition: {
					x: camera.position.x,
					y: camera.position.y,
					z: camera.position.z,
				},
				cameraUp: { x: camera.up.x, y: camera.up.y, z: camera.up.z },
				enabled: controls?.enabled,
				enableDamping: controls?.enableDamping,
				dampingFactor: controls?.dampingFactor,
			});

			// Get the initial focal point from controls (already set to splat center)
			const initialFocalPoint = controls
				? controls.target.clone()
				: { x: 0, y: 0, z: 0 };

			console.log(
				"[Animation Start] Initial focal point:",
				initialFocalPoint,
			);

			const animate = () => {
				// If user clicked and library's camera transition is active, don't interfere
				if (viewer.transitioningCameraTarget) {
					// Update our focal point ref to match the library's transitioning target
					const currentTarget = {
						x: viewer.controls.target.x,
						y: viewer.controls.target.y,
						z: viewer.controls.target.z,
					};
					focalPointRef.current = currentTarget;
					targetFocalPointRef.current = currentTarget;

					animationFrameRef.current = requestAnimationFrame(animate);
					return;
				}

				const elapsed = (Date.now() - startTime) / 1000;
				// Back and forth sway, not continuous rotation
				// Read from refs so values update live without restarting animation
				const angle =
					Math.sin(elapsed * swaySpeedRef.current) *
					swayAmountRef.current;

				// Handle smooth focal point transition
				const transition = focalPointTransitionRef.current;
				let focalPoint = focalPointRef.current;

				if (transition.active) {
					const now = Date.now();
					const progress = Math.min(
						(now - transition.startTime) / transition.duration,
						1,
					);
					// Smooth easing function
					const eased =
						progress < 0.5
							? 2 * progress * progress
							: 1 - Math.pow(-2 * progress + 2, 2) / 2;

					// Lerp focal point
					const from = transition.startFocalPoint;
					const to = targetFocalPointRef.current;
					focalPoint = {
						x: from.x + (to.x - from.x) * eased,
						y: from.y + (to.y - from.y) * eased,
						z: from.z + (to.z - from.z) * eased,
					};
					focalPointRef.current = focalPoint;

					// During transition, maintain the camera's initial offset
					// This prevents zoom - camera moves with the focal point
					camera.position.x =
						focalPoint.x + transition.cameraOffset.x;
					camera.position.y =
						focalPoint.y + transition.cameraOffset.y;
					camera.position.z =
						focalPoint.z + transition.cameraOffset.z;

					// DON'T update controls.target during transition - let it happen after
					// This prevents OrbitControls from calculating wrong spherical radius

					if (progress >= 1) {
						transition.active = false;
						// NOW update controls.target after camera is positioned correctly
						if (controls) {
							controls.target.set(
								focalPoint.x,
								focalPoint.y,
								focalPoint.z,
							);
						}
						console.log(
							"[GaussianSplatViewer] Focal point transition complete, controls.target updated",
						);
					}
				} else {
					// Smoothly interpolate distance when slider changes
					const targetDistance = cameraDistanceRef.current;
					const currentDistance = currentCameraDistanceRef.current;
					const distanceDiff = targetDistance - currentDistance;

					if (Math.abs(distanceDiff) > 0.001) {
						// Smooth interpolation (20% per frame)
						const oldDistance = currentCameraDistanceRef.current;
						currentCameraDistanceRef.current += distanceDiff * 0.2;

						if (Math.abs(distanceDiff) > 0.1) {
							console.log(
								"[Animation] Distance interpolating from",
								oldDistance.toFixed(3),
								"to",
								targetDistance.toFixed(3),
							);
						}
					} else {
						currentCameraDistanceRef.current = targetDistance;
					}

					// Normal orbital animation with interpolated distance
					camera.position.x =
						focalPoint.x +
						Math.sin(angle) * currentCameraDistanceRef.current;
					camera.position.z =
						focalPoint.z +
						-Math.cos(angle) * currentCameraDistanceRef.current;
					camera.position.y = focalPoint.y;
				}

				camera.lookAt(focalPoint.x, focalPoint.y, focalPoint.z);

				// Only update controls.target when NOT transitioning
				// During normal animation, keep it synced to prevent drift
				if (!transition.active && controls) {
					const oldTarget = {
						x: controls.target.x,
						y: controls.target.y,
						z: controls.target.z,
					};
					controls.target.set(
						focalPoint.x,
						focalPoint.y,
						focalPoint.z,
					);

					// Log if target changed significantly
					const targetChanged =
						Math.abs(oldTarget.x - focalPoint.x) > 0.001 ||
						Math.abs(oldTarget.y - focalPoint.y) > 0.001 ||
						Math.abs(oldTarget.z - focalPoint.z) > 0.001;
					if (targetChanged) {
						console.log(
							"[Animation] Controls.target changed from",
							oldTarget,
							"to",
							{
								x: focalPoint.x,
								y: focalPoint.y,
								z: focalPoint.z,
							},
						);
					}
				}

				animationFrameRef.current = requestAnimationFrame(animate);
			};

			// Set initial camera position relative to focal point, then start animation
			requestAnimationFrame(() => {
				const fp = focalPointRef.current;
				camera.position.set(
					fp.x,
					fp.y,
					fp.z - cameraDistanceRef.current,
				);
				camera.lookAt(fp.x, fp.y, fp.z);
				camera.updateProjectionMatrix();

				// Update controls to sync with new camera position
				if (controls) {
					controls.update();
				}

				console.log("[Animation Start] Camera positioned at:", {
					x: camera.position.x,
					y: camera.position.y,
					z: camera.position.z,
				});

				animate();
			});
		};

		startAnimation();

		return () => {
			// Clear any pending start timeout
			if (startTimeoutId !== null) {
				clearTimeout(startTimeoutId);
			}
			// Cancel animation frame
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
	onResetFocalPoint?: () => void;
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
	onResetFocalPoint,
}: MeshViewerUIProps) {
	const [showSettings, setShowSettings] = useState(false);

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
			{/* Button controls */}
			<div className="pointer-events-auto absolute bottom-4 right-4 flex gap-1">
				<TopBarButton
					icon={Sliders}
					onClick={() => setShowSettings(!showSettings)}
					title="Settings"
					active={showSettings}
					activeAccent={true}
				/>
				{onResetFocalPoint && (
					<TopBarButton
						icon={ArrowCounterClockwise}
						onClick={onResetFocalPoint}
						title="Reset focal point"
					/>
				)}
				<TopBarButton
					icon={autoRotate ? Pause : Play}
					onClick={() => setAutoRotate(!autoRotate)}
					title={autoRotate ? "Pause" : "Play"}
					active={autoRotate}
					activeAccent={true}
				/>
			</div>

			{/* Settings panel (only shown when button is clicked) */}
			{showSettings && (
				<div className="pointer-events-auto absolute bottom-16 right-4 flex flex-col gap-2 bg-app-box/95 backdrop-blur-lg border border-app-line rounded-lg p-3 shadow-lg min-w-[240px]">
					{/* Sway amount slider */}
					<div className="flex flex-col gap-1">
						<div className="flex items-center justify-between">
							<label className="text-xs text-ink-dull">
								Sway Amount
							</label>
							<span className="text-xs text-ink-dull font-mono">
								{swayAmount.toFixed(2)}
							</span>
						</div>
						<input
							type="range"
							min="0"
							max="0.5"
							step="0.01"
							value={swayAmount}
							onChange={(e) =>
								setSwayAmount(parseFloat(e.target.value))
							}
							className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
						/>
					</div>

					{/* Speed slider */}
					<div className="flex flex-col gap-1">
						<div className="flex items-center justify-between">
							<label className="text-xs text-ink-dull">
								Speed
							</label>
							<span className="text-xs text-ink-dull font-mono">
								{swaySpeed.toFixed(2)}
							</span>
						</div>
						<input
							type="range"
							min="0.1"
							max="2"
							step="0.1"
							value={swaySpeed}
							onChange={(e) =>
								setSwaySpeed(parseFloat(e.target.value))
							}
							className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
						/>
					</div>

					{/* Distance slider */}
					<div className="flex flex-col gap-1">
						<div className="flex items-center justify-between">
							<label className="text-xs text-ink-dull">
								Distance
							</label>
							<span className="text-xs text-ink-dull font-mono">
								{cameraDistance.toFixed(2)}
							</span>
						</div>
						<input
							type="range"
							min="0.2"
							max="1.5"
							step="0.05"
							value={cameraDistance}
							onChange={(e) =>
								setCameraDistance(parseFloat(e.target.value))
							}
							className="w-full h-1.5 bg-app-button rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:cursor-pointer"
						/>
					</div>
				</div>
			)}
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
	const [meshUrl, setMeshUrl] = useState<string | null>(splatUrl || null);
	const [isGaussianSplat, setIsGaussianSplat] = useState(!!splatUrl);
	const [splatFailed, setSplatFailed] = useState(false);
	const [shouldLoad, setShouldLoad] = useState(false);
	const [loading, setLoading] = useState(!splatUrl);
	const resetFocalPointRef = useRef<(() => void) | null>(null);
	const [internalCameraDistance, setInternalCameraDistance] =
		useState(cameraDistanceProp);

	// Use props for control values, but use internal state for distance (can be overridden by raycast)
	const autoRotate = autoRotateProp;
	const swayAmount = swayAmountProp;
	const swaySpeed = swaySpeedProp;
	const cameraDistance = internalCameraDistance;

	// Sync internal distance with prop changes (unless we've overridden it)
	useEffect(() => {
		setInternalCameraDistance(cameraDistanceProp);
	}, [cameraDistanceProp]);

	// Notify parent when controls change
	useEffect(() => {
		onControlsChange?.({
			autoRotate,
			swayAmount,
			swaySpeed,
			cameraDistance: internalCameraDistance,
			isGaussianSplat,
			onResetFocalPoint: resetFocalPointRef.current || undefined,
		});
	}, [
		isGaussianSplat,
		autoRotate,
		swayAmount,
		swaySpeed,
		internalCameraDistance,
		onControlsChange,
	]);

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
					onResetReady={(resetFn) => {
						resetFocalPointRef.current = resetFn;
						// Trigger controls change update to notify parent
						onControlsChange?.({
							autoRotate,
							swayAmount,
							swaySpeed,
							cameraDistance: internalCameraDistance,
							isGaussianSplat,
							onResetFocalPoint: resetFn,
						});
					}}
					onDistanceCalculated={(distance) => {
						setInternalCameraDistance(distance);
					}}
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
