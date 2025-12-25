import { motion, AnimatePresence } from "framer-motion";
import { Ball } from "@sd/assets/images";
import { CircleNotch } from "@phosphor-icons/react";
import { useState, useEffect } from "react";
import { usePlatform } from "../platform";
import Orb from "./Orb";

export function DaemonStartupOverlay({ show }: { show: boolean }) {
	const platform = usePlatform();
	const [version, setVersion] = useState<string>("2.0.0");
	const isDev = import.meta.env.DEV;

	// Get version from platform abstraction
	useEffect(() => {
		const getVersion = async () => {
			try {
				if (platform.getAppVersion) {
					const appVersion = await platform.getAppVersion();
					setVersion(appVersion);
				}
			} catch (e) {
				// Fallback if platform doesn't support version or API fails
				setVersion("2.0.0-pre.1");
			}
		};
		getVersion();
	}, [platform]);

	const versionText = isDev ? `${version} (dev)` : version;

	return (
		<AnimatePresence>
			{show && (
				<motion.div
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{
						duration: 0.6,
						ease: "easeInOut",
					}}
					className="fixed inset-0 z-[9999] flex items-center justify-center bg-black"
				>
					{/* Animated orb with ball */}
					<motion.div
						initial={{ scale: 0.8, opacity: 0 }}
						animate={{ scale: 1, opacity: 1 }}
						exit={{ scale: 0.95, opacity: 0 }}
						transition={{ duration: 0.6, ease: "easeOut" }}
						className="relative w-64 h-64"
					>
						{/* Ball image - behind the orb */}
						<div className="absolute inset-[8%] z-0">
							<img
								src={Ball}
								alt="Spacedrive"
								className="w-full h-full object-contain select-none"
								draggable={false}
							/>
						</div>
						{/* Orb animation - inset to make it smaller */}
						<div className="absolute inset-[15%] z-10">
							<Orb
								hue={-30}
								hoverIntensity={0}
								rotateOnHover={false}
								forceHoverState={true}
							/>
						</div>
					</motion.div>

					{/* Loading text - bottom right */}
					<motion.div
						initial={{ opacity: 0, x: 10 }}
						animate={{ opacity: 1, x: 0 }}
						exit={{ opacity: 0, x: -10 }}
						transition={{ duration: 0.5, delay: 0.3 }}
						className="fixed bottom-6 right-6 flex items-center gap-3"
					>
						<CircleNotch
							className="size-5 animate-spin text-white"
							weight="bold"
						/>
						<div className="flex flex-col">
							<p className="text-lg font-bold text-white">
								Starting Spacedrive
							</p>
							<p className="text-sm text-white/50">
								v{versionText}
							</p>
						</div>
					</motion.div>
				</motion.div>
			)}
		</AnimatePresence>
	);
}
