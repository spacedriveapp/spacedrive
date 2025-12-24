import { motion, AnimatePresence } from "framer-motion";
import { SD } from "@sd/assets/icons";
import { Loader } from "@sd/ui";

export function DaemonStartingOverlay() {
	return (
		<AnimatePresence>
			<motion.div
				initial={{ opacity: 0 }}
				animate={{ opacity: 1 }}
				exit={{ opacity: 0 }}
				transition={{ duration: 0.2 }}
				className="fixed inset-0 z-[9999] flex items-center justify-center bg-app"
			>
				<div className="flex flex-col items-center justify-center gap-6">
					<motion.img
						src={SD}
						alt="Spacedrive"
						className="size-32 select-none"
						draggable={false}
						initial={{ scale: 0.9, opacity: 0 }}
						animate={{ scale: 1, opacity: 1 }}
						transition={{ duration: 0.3, delay: 0.1 }}
					/>
					<Loader className="size-8" />
				</div>
			</motion.div>
		</AnimatePresence>
	);
}
