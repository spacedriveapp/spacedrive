import { motion, AnimatePresence } from "framer-motion";
import { AppLogo } from "@sd/assets/images";

export function DaemonStartupOverlay({ show }: { show: boolean }) {
	return (
		<AnimatePresence>
			{show && (
				<motion.div
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{ duration: 0.3 }}
					className="fixed inset-0 z-[9999] flex flex-col items-center justify-center bg-app"
				>
					{/* Logo with subtle pulse animation */}
					<motion.div
						initial={{ scale: 0.9, opacity: 0 }}
						animate={{ scale: 1, opacity: 1 }}
						transition={{ duration: 0.5, ease: "easeOut" }}
						className="relative"
					>
						<img
							src={AppLogo}
							alt="Spacedrive"
							className="size-24 select-none"
							draggable={false}
						/>
						
						{/* Subtle glow effect behind logo */}
						<div className="absolute inset-0 -z-10 blur-3xl">
							<div className="size-full rounded-full bg-accent/20" />
						</div>
					</motion.div>

					{/* Loading indicator */}
					<motion.div
						initial={{ opacity: 0, y: 10 }}
						animate={{ opacity: 1, y: 0 }}
						transition={{ duration: 0.5, delay: 0.2 }}
						className="mt-8 flex flex-col items-center gap-4"
					>
						{/* Animated dots loader */}
						<div className="flex gap-1.5">
							{[0, 1, 2].map((i) => (
								<motion.div
									key={i}
									className="size-2 rounded-full bg-accent"
									animate={{
										scale: [1, 1.2, 1],
										opacity: [0.5, 1, 0.5],
									}}
									transition={{
										duration: 1,
										repeat: Infinity,
										delay: i * 0.15,
										ease: "easeInOut",
									}}
								/>
							))}
						</div>

						<p className="text-sm text-ink-dull">
							Starting Spacedrive...
						</p>
					</motion.div>
				</motion.div>
			)}
		</AnimatePresence>
	);
}
