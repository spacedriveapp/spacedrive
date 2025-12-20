import { useState, useEffect, useRef } from "react";
import { Canvas, useFrame } from "@react-three/fiber";
import { XR, VRButton, Controllers } from "@react-three/xr";
import { Html } from "@react-three/drei";
import { DemoWindow } from "@sd/interface";
import { useSpacedriveClient, useCoreQuery } from "@sd/ts-client/hooks";
import * as THREE from "three";
import { FileExplorer } from "./FileExplorer";

// Native VR display for Spacedrive library info
function SpacedriveLibraryDisplay() {
	const client = useSpacedriveClient();
	const meshRef = useRef<THREE.Mesh>(null);

	// Use the proper hook to fetch library list
	const {
		data: libraries,
		error,
		isLoading,
	} = useCoreQuery({
		type: "libraries.list",
		input: {
			include_stats: true, // Get stats for display
		},
	});

	// Log connection state changes
	useEffect(() => {
		console.log("[SpacedriveVR] Client state:", { client });
		console.log("[SpacedriveVR] Query state:", {
			libraries,
			error,
			isLoading,
		});
		if (libraries) {
			console.log("[SpacedriveVR] Libraries data:", libraries);
		}
		if (error) {
			console.error("[SpacedriveVR] Error fetching libraries:", error);
			// Show full error object
			if (error instanceof Error) {
				console.error("[SpacedriveVR] Error message:", error.message);
				console.error("[SpacedriveVR] Error stack:", error.stack);
			}
		}
	}, [client, libraries, error, isLoading]);

	// Update canvas texture with library info
	useEffect(() => {
		const canvas = document.createElement("canvas");
		canvas.width = 2048;
		canvas.height = 1536;
		const ctx = canvas.getContext("2d");

		if (!ctx) return;

		// Background
		const gradient = ctx.createLinearGradient(
			0,
			0,
			canvas.width,
			canvas.height,
		);
		gradient.addColorStop(0, "#1a1a2e");
		gradient.addColorStop(1, "#16213e");
		ctx.fillStyle = gradient;
		ctx.fillRect(0, 0, canvas.width, canvas.height);

		// Border
		ctx.strokeStyle = "#6366f1";
		ctx.lineWidth = 8;
		ctx.strokeRect(20, 20, canvas.width - 40, canvas.height - 40);

		// Title
		ctx.fillStyle = "white";
		ctx.font = "bold 100px sans-serif";
		ctx.textAlign = "center";
		ctx.fillText("SPACEDRIVE VR", canvas.width / 2, 150);

		// Connection status - show based on query state
		ctx.font = "60px sans-serif";
		if (error) {
			ctx.fillStyle = "#ef4444";
			ctx.fillText("❌ Connection Failed", canvas.width / 2, 300);
		} else if (isLoading) {
			ctx.fillStyle = "#fbbf24";
			ctx.fillText("🔄 Connecting...", canvas.width / 2, 300);
		} else if (libraries) {
			ctx.fillStyle = "#22c55e";
			ctx.fillText("✅ Connected to Daemon", canvas.width / 2, 300);
		} else {
			ctx.fillStyle = "#94a3b8";
			ctx.fillText("⏳ Initializing...", canvas.width / 2, 300);
		}

		// Library info
		if (error) {
			ctx.fillStyle = "#ef4444";
			ctx.font = "50px sans-serif";
			ctx.fillText("❌ Error Loading Libraries", canvas.width / 2, 480);

			ctx.font = "35px monospace";
			ctx.fillStyle = "#fca5a5";
			const errorStr =
				error instanceof Error ? error.message : String(error);

			// Split long error messages
			const maxChars = 50;
			let yPos = 570;
			for (let i = 0; i < errorStr.length; i += maxChars) {
				ctx.fillText(
					errorStr.substring(i, i + maxChars),
					canvas.width / 2,
					yPos,
				);
				yPos += 45;
			}

			// Troubleshooting steps
			ctx.font = "32px sans-serif";
			ctx.fillStyle = "#fbbf24";
			yPos += 30;
			ctx.fillText("Troubleshooting:", canvas.width / 2, yPos);

			ctx.font = "28px sans-serif";
			ctx.fillStyle = "#94a3b8";
			yPos += 45;
			ctx.fillText(
				"1. Visit https://192.168.0.91:8080/test",
				canvas.width / 2,
				yPos,
			);
			yPos += 40;
			ctx.fillText(
				"2. Test WebSocket connection",
				canvas.width / 2,
				yPos,
			);
			yPos += 40;
			ctx.fillText("3. Verify IP in src/App.tsx", canvas.width / 2, yPos);
		} else if (isLoading) {
			ctx.fillStyle = "#94a3b8";
			ctx.font = "50px sans-serif";
			ctx.fillText("Loading libraries...", canvas.width / 2, 500);
		} else if (libraries && libraries.length > 0) {
			ctx.fillStyle = "#a855f7";
			ctx.font = "70px sans-serif";
			ctx.fillText(
				`Found ${libraries.length} ${libraries.length === 1 ? "Library" : "Libraries"}`,
				canvas.width / 2,
				420,
			);

			// Display each library
			let yPos = 580;
			libraries.forEach((lib: any, index: number) => {
				// Library name
				ctx.fillStyle = "#e2e8f0";
				ctx.font = "bold 55px sans-serif";
				ctx.fillText(
					`${index + 1}. ${lib.name || "Unnamed Library"}`,
					canvas.width / 2,
					yPos,
				);

				// Stats if available
				if (lib.stats) {
					ctx.font = "38px sans-serif";
					ctx.fillStyle = "#22c55e";
					const files = lib.stats.total_files.toLocaleString();
					const locations = lib.stats.location_count;
					ctx.fillText(
						`📁 ${files} files  •  📍 ${locations} locations`,
						canvas.width / 2,
						yPos + 55,
					);
				}

				yPos += 150;
			});
		} else {
			ctx.fillStyle = "#fbbf24";
			ctx.font = "50px sans-serif";
			ctx.fillText("No libraries found", canvas.width / 2, 500);
		}

		// Create texture
		const texture = new THREE.CanvasTexture(canvas);
		texture.needsUpdate = true;

		// Apply to mesh
		if (meshRef.current) {
			(meshRef.current.material as THREE.MeshBasicMaterial).map = texture;
			(meshRef.current.material as THREE.MeshBasicMaterial).needsUpdate =
				true;
		}
	}, [libraries, error, isLoading]);

	return (
		<mesh ref={meshRef} position={[0, 1.6, -2]}>
			<planeGeometry args={[4, 3]} />
			<meshBasicMaterial side={THREE.DoubleSide} />
		</mesh>
	);
}

// Wrapper for Spacedrive interface in VR (not currently used, will integrate later)
function VRInterface() {
	const client = useSpacedriveClient();

	return (
		<div
			style={{
				width: "1400px",
				height: "900px",
				background:
					"linear-gradient(135deg, #ff0080 0%, #ff8c00 50%, #40e0d0 100%)",
				display: "flex",
				flexDirection: "column",
				alignItems: "center",
				justifyContent: "center",
				color: "#fff",
				fontFamily: "system-ui",
				padding: "40px",
				boxSizing: "border-box",
			}}
		>
			<h1
				style={{
					fontSize: "80px",
					marginBottom: "30px",
					fontWeight: "bold",
					textShadow: "0 4px 8px rgba(0,0,0,0.5)",
				}}
			>
				🚀 HTML IS WORKING! 🚀
			</h1>
			<p
				style={{
					fontSize: "40px",
					marginBottom: "20px",
					textShadow: "0 2px 4px rgba(0,0,0,0.5)",
				}}
			>
				Spacedrive VR Interface Test
			</p>
			<div
				style={{
					fontSize: "30px",
					background: "rgba(0,0,0,0.3)",
					padding: "20px",
					borderRadius: "12px",
				}}
			>
				Client Status: {client ? "✅ Connected" : "❌ Not Connected"}
			</div>
		</div>
	);
}

// Old placeholder (keeping for reference)
function PlaceholderInterface_OLD({
	onOpenFile,
}: {
	onOpenFile: (file: any) => void;
}) {
	return (
		<div
			style={{
				width: 1400,
				height: 900,
				background: "linear-gradient(135deg, #1a1a2e 0%, #16213e 100%)",
				borderRadius: 16,
				padding: 32,
				color: "#fff",
				fontFamily: "system-ui",
				boxShadow: "0 20px 60px rgba(0,0,0,0.5)",
			}}
		>
			<h1 style={{ fontSize: 48, marginBottom: 24 }}>Spacedrive VR</h1>
			<p style={{ fontSize: 20, marginBottom: 32, opacity: 0.8 }}>
				Your files in immersive space
			</p>

			<div
				style={{
					display: "grid",
					gridTemplateColumns: "repeat(4, 1fr)",
					gap: 16,
				}}
			>
				{[
					{ name: "bunny.ply", type: "Gaussian Splat" },
					{ name: "scene.ply", type: "3D Mesh" },
					{ name: "video360.mp4", type: "Spatial Video" },
					{ name: "photo.jpg", type: "Spatial Photo" },
				].map((file, i) => (
					<button
						key={i}
						onClick={() => onOpenFile(file)}
						style={{
							background: "#2d3748",
							border: "2px solid #4a5568",
							borderRadius: 12,
							padding: 24,
							color: "#fff",
							cursor: "pointer",
							fontSize: 16,
							transition: "all 0.2s",
						}}
						onMouseOver={(e) => {
							e.currentTarget.style.background = "#374151";
							e.currentTarget.style.borderColor = "#6366f1";
						}}
						onMouseOut={(e) => {
							e.currentTarget.style.background = "#2d3748";
							e.currentTarget.style.borderColor = "#4a5568";
						}}
					>
						<div style={{ fontSize: 40, marginBottom: 8 }}>📦</div>
						<div style={{ fontWeight: 600 }}>{file.name}</div>
						<div
							style={{ fontSize: 14, opacity: 0.6, marginTop: 4 }}
						>
							{file.type}
						</div>
					</button>
				))}
			</div>

			<div
				style={{
					marginTop: 48,
					padding: 24,
					background: "rgba(99, 102, 241, 0.1)",
					border: "2px solid rgba(99, 102, 241, 0.3)",
					borderRadius: 12,
				}}
			>
				<h2 style={{ fontSize: 24, marginBottom: 12 }}>
					Getting Started
				</h2>
				<ul style={{ fontSize: 16, lineHeight: 1.8, opacity: 0.9 }}>
					<li>Point with your VR controller to interact</li>
					<li>Click a file to enter immersive mode</li>
					<li>Press B to return to this interface</li>
				</ul>
			</div>
		</div>
	);
}

// Immersive 3D viewer component
function ImmersiveViewer({ file, onBack }: { file: any; onBack: () => void }) {
	return (
		<group>
			{/* Back button floating in space */}
			<Html position={[-2, 2, -1]} center>
				<button
					onClick={onBack}
					style={{
						background: "#6366f1",
						border: "none",
						borderRadius: 8,
						padding: "12px 24px",
						color: "#fff",
						fontSize: 18,
						cursor: "pointer",
						fontWeight: 600,
						boxShadow: "0 4px 12px rgba(99, 102, 241, 0.5)",
					}}
				>
					← Back
				</button>
			</Html>

			{/* Placeholder 3D content - replace with actual MeshViewer */}
			<mesh position={[0, 1.6, -2]}>
				<boxGeometry args={[1, 1, 1]} />
				<meshStandardMaterial color="#6366f1" />
			</mesh>

			<Html position={[0, 0.8, -2]} center>
				<div
					style={{ color: "#fff", textAlign: "center", fontSize: 20 }}
				>
					<div style={{ fontWeight: 600, marginBottom: 8 }}>
						{file.name}
					</div>
					<div style={{ opacity: 0.7 }}>{file.type}</div>
					<div style={{ marginTop: 16, opacity: 0.5, fontSize: 14 }}>
						(3D viewer will render here)
					</div>
				</div>
			</Html>
		</group>
	);
}

export function SpacedriveVR() {
	const [immersiveFile, setImmersiveFile] = useState<any>(null);

	return (
		<div
			style={{
				width: "100vw",
				height: "100vh",
				position: "fixed",
				top: 0,
				left: 0,
			}}
		>
			{/* VR Entry Button - outside Canvas */}
			<VRButton />

			{/* Welcome Screen */}
			<div
				style={{
					position: "fixed",
					top: "50%",
					left: "50%",
					transform: "translate(-50%, -50%)",
					textAlign: "center",
					color: "#fff",
					zIndex: 999,
					pointerEvents: "none",
				}}
			>
				<h1
					style={{
						fontSize: "56px",
						marginBottom: "16px",
						fontWeight: "700",
					}}
				>
					Spacedrive VR
				</h1>
				<p style={{ fontSize: "20px", opacity: 0.7 }}>
					Your files in immersive space
				</p>
			</div>

			<Canvas>
				<XR>
					{/* Much brighter lighting */}
					<ambientLight intensity={1.5} />
					<pointLight position={[0, 3, 0]} intensity={2} />
					<pointLight position={[2, 1, 2]} intensity={1} />
					<pointLight position={[-2, 1, 2]} intensity={1} />

					{/* VR Controllers with ray pointers */}
					<Controllers rayMaterial={{ color: "#a855f7" }} />

					{/* Grid floor for reference */}
					<gridHelper
						args={[20, 20, "#444", "#222"]}
						position={[0, 0, 0]}
					/>

					{/* Floating Spacedrive Interface */}
					{!immersiveFile && <FileExplorer />}

					{/* Reference purple balls */}
					<mesh position={[1, 1.6, -1]}>
						<sphereGeometry args={[0.15, 32, 32]} />
						<meshStandardMaterial
							color="#a855f7"
							emissive="#a855f7"
							emissiveIntensity={0.8}
						/>
					</mesh>
					<mesh position={[-1, 1.6, -1]}>
						<sphereGeometry args={[0.15, 32, 32]} />
						<meshStandardMaterial
							color="#8b5cf6"
							emissive="#8b5cf6"
							emissiveIntensity={0.8}
						/>
					</mesh>

					{/* Immersive 3D Content */}
					{immersiveFile && (
						<ImmersiveViewer
							file={immersiveFile}
							onBack={() => setImmersiveFile(null)}
						/>
					)}
				</XR>
			</Canvas>
		</div>
	);
}
