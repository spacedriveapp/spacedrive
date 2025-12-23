import { ShinyButton } from "@sd/ui";
import { SpacedriveProvider } from "./context";
import { useLibraries } from "./hooks/useLibraries";
import { useAllEvents } from "./hooks/useEvent";
import { useState } from "react";
import type { SpacedriveClient } from "./context";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { usePlatform } from "./platform";

// Type for library info (will be properly typed later)
interface LibraryInfo {
	id: string;
	name: string;
	path?: string;
	stats?: {
		total_files?: number;
		total_size?: number;
		location_count?: number;
	};
}

interface AppProps {
	client: SpacedriveClient;
}

function LibrariesView() {
	const { data: libraries, isLoading, error, refetch } = useLibraries(true);
	const [lastEvent, setLastEvent] = useState<any>(null);
	const [eventCount, setEventCount] = useState(0);
	const [windowError, setWindowError] = useState<string | null>(null);

	const platform = usePlatform();

	// Listen to all core events
	useAllEvents((event) => {
		setLastEvent(event);
		setEventCount((c) => c + 1);
	});

	async function openWindow(windowType: string, params?: any) {
		try {
			setWindowError(null);
			if (!platform.showWindow) {
				throw new Error(
					"Window management not available on this platform",
				);
			}

			const windowDef =
				windowType === "Settings"
					? { type: "Settings", page: params }
					: { type: windowType, ...params };

			await platform.showWindow(windowDef);
			setLastEvent({ success: "Window opened", type: windowType });
		} catch (err) {
			const errMsg = err instanceof Error ? err.message : String(err);
			setWindowError(errMsg);
			setLastEvent({ error: "Window open failed", message: errMsg });
		}
	}

	const status = isLoading ? "connecting" : error ? "error" : "connected";

	return (
		<div className="h-screen bg-gray-950 text-white overflow-hidden flex flex-col">
			{/* Header */}
			<div className="border-b border-gray-800 bg-gray-900/50 backdrop-blur">
				<div className="px-6 py-4">
					<div className="flex items-center justify-between">
						<div>
							<h1 className="text-2xl font-bold">
								Spacedrive V2
							</h1>
							<p className="text-sm text-gray-400">
								Multi-window Architecture Demo
							</p>
						</div>
						<div className="flex items-center gap-2">
							<div
								className={`w-2 h-2 rounded-full ${
									status === "connecting"
										? "bg-yellow-500 animate-pulse"
										: status === "connected"
											? "bg-green-500"
											: "bg-red-500"
								}`}
							/>
							<span className="text-xs text-gray-400">
								{status === "connected"
									? "Connected"
									: status === "error"
										? "Error"
										: "Loading"}
							</span>
						</div>
					</div>
				</div>
			</div>

			{/* Main Content */}
			<div className="flex-1 overflow-y-auto">
				<div className="max-w-6xl mx-auto p-6 space-y-6">
					{/* Libraries Grid */}
					{status === "connected" && libraries && (
						<div>
							<h2 className="text-lg font-semibold mb-4">
								Libraries
							</h2>
							<div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
								{libraries.length === 0 ? (
									<div className="col-span-full bg-gray-900 rounded-lg p-8 border border-gray-800 text-center">
										<p className="text-gray-400">
											No libraries found
										</p>
									</div>
								) : (
									libraries.map((lib: LibraryInfo) => (
										<div
											key={lib.id}
											className="bg-gray-900 rounded-lg p-4 border border-gray-800 hover:border-gray-700 transition-colors"
										>
											<h3 className="font-semibold mb-3">
												{lib.name}
											</h3>
											<div className="space-y-2 text-sm">
												<div className="flex justify-between text-gray-400">
													<span>Files</span>
													<span className="text-white">
														{lib.stats?.total_files?.toLocaleString() ||
															0}
													</span>
												</div>
												<div className="flex justify-between text-gray-400">
													<span>Size</span>
													<span className="text-white">
														{formatBytes(
															lib.stats
																?.total_size ||
																0,
														)}
													</span>
												</div>
												{lib.path && (
													<div className="pt-2 border-t border-gray-800">
														<p
															className="text-xs text-gray-500 truncate"
															title={lib.path}
														>
															{lib.path}
														</p>
													</div>
												)}
											</div>
										</div>
									))
								)}
							</div>
						</div>
					)}

					{/* Events Section */}
					<div>
						<div className="flex items-center justify-between mb-4">
							<h2 className="text-lg font-semibold">
								Live Events
							</h2>
							<span className="text-xs bg-gray-800 px-2 py-1 rounded-full">
								{eventCount} received
							</span>
						</div>
						<div className="bg-gray-900 rounded-lg border border-gray-800 p-4">
							{lastEvent ? (
								<pre className="text-xs text-gray-400 overflow-auto max-h-48 font-mono">
									{JSON.stringify(lastEvent, null, 2)}
								</pre>
							) : (
								<p className="text-gray-500 text-sm text-center py-4">
									Waiting for events...
								</p>
							)}
						</div>
					</div>

					{/* Window Controls */}
					<div>
						<h2 className="text-lg font-semibold mb-4">
							Window Management
						</h2>
						<div className="grid grid-cols-2 md:grid-cols-4 gap-3">
							<ShinyButton
								variant="accent"
								onClick={() => refetch()}
								disabled={isLoading}
							>
								Refresh
							</ShinyButton>
							<ShinyButton
								onClick={() =>
									openWindow("Settings", "general")
								}
							>
								Settings
							</ShinyButton>
							<ShinyButton
								onClick={() =>
									openWindow("Inspector", {
										item_id: "test-123",
									})
								}
							>
								Inspector
							</ShinyButton>
							<ShinyButton
								onClick={() => openWindow("FloatingControls")}
							>
								Floating Controls
							</ShinyButton>
						</div>
						{windowError && (
							<div className="mt-3 bg-red-900/20 border border-red-500 rounded-lg p-3">
								<p className="text-red-400 text-sm">
									{windowError}
								</p>
							</div>
						)}
					</div>
				</div>
			</div>

			{error && (
				<div className="absolute inset-0 bg-black/50 backdrop-blur flex items-center justify-center">
					<div className="bg-gray-900 rounded-lg p-6 border border-red-500 max-w-md">
						<h2 className="text-xl font-bold mb-2 text-red-400">
							Error
						</h2>
						<p className="text-sm text-gray-300">{error.message}</p>
					</div>
				</div>
			)}
		</div>
	);
}

export function DemoWindow({ client }: AppProps) {
	return (
		<SpacedriveProvider client={client}>
			<LibrariesView />
			<ReactQueryDevtools
				initialIsOpen={false}
				buttonPosition="bottom-right"
			/>
		</SpacedriveProvider>
	);
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}
