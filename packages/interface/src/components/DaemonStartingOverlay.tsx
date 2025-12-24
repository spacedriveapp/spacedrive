import { motion } from "framer-motion";
import { AppLogo } from "@sd/assets/images";

export function DaemonStartingOverlay() {
	return (
		<div className="fixed inset-0 z-[9999] flex items-center justify-center bg-app">
			<div className="flex flex-col items-center gap-6">
				<motion.img
					src={AppLogo}
					alt="Spacedrive"
					className="size-32 select-none"
					draggable={false}
					initial={{ opacity: 0, scale: 0.9 }}
					animate={{ opacity: 1, scale: 1 }}
					transition={{
						duration: 0.3,
						ease: "easeOut"
					}}
				/>
				
				<motion.div
					className="flex flex-col items-center gap-3"
					initial={{ opacity: 0, y: 10 }}
					animate={{ opacity: 1, y: 0 }}
					transition={{
						duration: 0.4,
						delay: 0.2,
						ease: "easeOut"
					}}
				>
					<div className="flex items-center gap-2">
						<motion.div
							className="size-2 rounded-full bg-accent"
							animate={{
								opacity: [0.4, 1, 0.4],
								scale: [0.8, 1, 0.8]
							}}
							transition={{
								duration: 1.5,
								repeat: Infinity,
								ease: "easeInOut"
							}}
						/>
						<span className="text-sm text-ink-dull">Starting Spacedrive...</span>
					</div>
				</motion.div>
			</div>
		</div>
	);
}
