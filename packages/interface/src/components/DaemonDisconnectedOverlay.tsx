import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Copy } from "@phosphor-icons/react";
import { useDaemonStatus } from "../hooks/useDaemonStatus";
import { usePlatform } from "../platform";
import { Button } from "@sd/ui";
import folderIcon from "@sd/assets/icons/FolderNoSpace.png";

function CLICommand({
	command,
	description,
}: {
	command: string;
	description: string;
}) {
	const copyToClipboard = () => {
		navigator.clipboard.writeText(command);
	};

	return (
		<div className="group relative rounded-md px-3 py-2">
			<code className="text-xs text-accent">{command}</code>
			<p className="mt-1 text-xs text-ink-dull">{description}</p>
			<button
				onClick={copyToClipboard}
				className="absolute right-2 top-2 rounded p-1 opacity-0 transition-opacity hover:bg-app-hover group-hover:opacity-100"
				title="Copy to clipboard"
			>
				<Copy className="size-3 text-ink-dull" weight="bold" />
			</button>
		</div>
	);
}

export function DaemonDisconnectedOverlay({
	forceShow = false,
	daemonStatus,
}: {
	forceShow?: boolean;
	daemonStatus: ReturnType<typeof useDaemonStatus>;
}) {
	const {
		isConnected,
		isChecking,
		isInstalled,
		startDaemon,
		installAndStartDaemon,
	} = daemonStatus;
	const [installAsService, setInstallAsService] = useState(isInstalled);
	const platform = usePlatform();

	// Update checkbox when installation state changes
	useEffect(() => {
		console.log(
			"[DaemonDisconnectedOverlay] isInstalled changed to:",
			isInstalled,
		);
		setInstallAsService(isInstalled);
	}, [isInstalled]);

	// Log checkbox state changes
	useEffect(() => {
		console.log(
			"[DaemonDisconnectedOverlay] installAsService checkbox state:",
			installAsService,
		);
	}, [installAsService]);

	const shouldShow = forceShow || !isConnected;

	return (
		<AnimatePresence>
			{shouldShow && (
				<motion.div
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{ duration: 0.2 }}
					className="fixed inset-0 z-[9999] flex items-center justify-center backdrop-blur-lg bg-black/50"
				>
					<div className="fixed right-4 top-4 flex items-center gap-2">
						<div className="flex items-center gap-2 rounded-full border border-app-line bg-app-box px-3 py-1.5 text-xs font-medium">
							<div
								className={`size-2 rounded-full ${
									isChecking
										? "bg-yellow-500"
										: isConnected
											? "bg-green-500"
											: "bg-red-500"
								} animate-pulse`}
							/>
							<span className="text-ink-dull">
								{isChecking
									? "Starting..."
									: isConnected
										? "Connected"
										: "Disconnected"}
							</span>
						</div>

						<div className="flex items-center gap-2 rounded-full border border-app-line bg-app-box px-3 py-1.5 text-xs font-medium">
							<div
								className={`size-2 rounded-full ${
									isInstalled ? "bg-accent" : "bg-gray-500"
								}`}
							/>
							<span className="text-ink-dull">
								{isInstalled ? "Persistent" : "Temporary"}
							</span>
						</div>
					</div>

					<div className="flex max-w-4xl gap-8 rounded-lg border border-app-line p-8 shadow-2xl">
						<div className="flex flex-1 flex-col items-center justify-center gap-6 px-12">
							<img
								src={folderIcon}
								alt="Spacedrive folder icon"
								className="size-32 select-none"
								draggable={false}
							/>

							<div className="flex flex-col items-center gap-2 text-center">
								<h1 className="text-2xl font-bold text-ink">
									Daemon Disconnected
								</h1>
								<p className="max-w-xs text-sm leading-relaxed text-ink-dull">
									The Spacedrive daemon is required for the
									app to function. It runs in the background,
									managing your libraries, indexing files, and
									syncing data across devices.
								</p>
							</div>

							<div className="flex flex-col items-center gap-3">
								<label className="flex cursor-pointer items-center gap-2 text-sm text-ink">
									<input
										type="checkbox"
										checked={installAsService}
										onChange={async (e) => {
											const shouldInstall =
												e.target.checked;
											setInstallAsService(shouldInstall);

											if (shouldInstall) {
												const success =
													await installAndStartDaemon();
												if (!success) {
													setInstallAsService(false);
												}
											} else {
												try {
													await platform.uninstallDaemonService?.();
												} catch (error) {
													console.error(
														"Failed to uninstall daemon service:",
														error,
													);
													setInstallAsService(true);
												}
											}
										}}
										className="size-4 cursor-pointer rounded border-app-line bg-app accent-accent"
									/>
									<span>Install as persistent service</span>
								</label>

								<div className="flex items-center gap-2">
									<Button variant="gray">Help</Button>
									<Button
										onClick={startDaemon}
										disabled={isChecking}
										variant="accent"
									>
										{isChecking
											? "Starting daemon..."
											: "Restart Daemon"}
									</Button>
								</div>
							</div>
						</div>

						<div className="flex w-80 flex-col gap-3 rounded-lg border border-app-line p-4">
							<span className="text-xs font-medium text-ink-dull">
								CLI Commands
							</span>

							<div className="space-y-2">
								<CLICommand
									command="sd start"
									description="Start the daemon in background mode"
								/>
								<CLICommand
									command="sd start --foreground"
									description="Start the daemon in foreground mode (see logs)"
								/>
								<CLICommand
									command="sd stop"
									description="Stop the daemon gracefully"
								/>
								<CLICommand
									command="sd restart"
									description="Restart the daemon"
								/>
								<CLICommand
									command="sd daemon install"
									description="Install daemon to start automatically on login (macOS/Linux)"
								/>
							</div>
						</div>
					</div>
				</motion.div>
			)}
		</AnimatePresence>
	);
}
