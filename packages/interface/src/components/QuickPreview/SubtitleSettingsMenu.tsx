import { motion, AnimatePresence } from 'framer-motion';
import type { SubtitleSettings } from './Subtitles';

interface SubtitleSettingsMenuProps {
	isOpen: boolean;
	settings: SubtitleSettings;
	onSettingsChange: (settings: SubtitleSettings) => void;
	onClose: () => void;
}

export function SubtitleSettingsMenu({
	isOpen,
	settings,
	onSettingsChange,
	onClose
}: SubtitleSettingsMenuProps) {
	return (
		<AnimatePresence>
			{isOpen && (
				<>
					{/* Backdrop */}
					<div
						className="fixed inset-0 z-10"
						onClick={onClose}
					/>

					{/* Settings Menu */}
					<motion.div
						initial={{ opacity: 0, y: 10 }}
						animate={{ opacity: 1, y: 0 }}
						exit={{ opacity: 0, y: 10 }}
						transition={{ duration: 0.15 }}
						className="absolute bottom-20 right-6 z-20 w-72 rounded-lg border border-app-line bg-sidebar-box/95 p-4 backdrop-blur-xl shadow-2xl"
						onClick={(e) => e.stopPropagation()}
					>
						<h3 className="mb-4 text-sm font-semibold text-ink">Subtitle Settings</h3>

						<div className="space-y-4">
							{/* Font Size */}
							<div>
								<label className="mb-2 flex items-center justify-between text-xs text-ink-dull">
									<span>Font Size</span>
									<span className="text-ink">{Math.round(settings.fontSize * 100)}%</span>
								</label>
								<input
									type="range"
									min="0.8"
									max="2.5"
									step="0.1"
									value={settings.fontSize}
									onChange={(e) =>
										onSettingsChange({
											...settings,
											fontSize: parseFloat(e.target.value)
										})
									}
									className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-sidebar-line [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:shadow-lg"
								/>
							</div>

							{/* Background Opacity */}
							<div>
								<label className="mb-2 flex items-center justify-between text-xs text-ink-dull">
									<span>Background Opacity</span>
									<span className="text-ink">{Math.round(settings.backgroundOpacity * 100)}%</span>
								</label>
								<input
									type="range"
									min="0"
									max="1"
									step="0.1"
									value={settings.backgroundOpacity}
									onChange={(e) =>
										onSettingsChange({
											...settings,
											backgroundOpacity: parseFloat(e.target.value)
										})
									}
									className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-sidebar-line [&::-webkit-slider-thumb]:size-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-accent [&::-webkit-slider-thumb]:shadow-lg"
								/>
							</div>

							{/* Position */}
							<div>
								<label className="mb-2 block text-xs text-ink-dull">Position</label>
								<div className="flex gap-2">
									<button
										onClick={() =>
											onSettingsChange({
												...settings,
												position: 'bottom'
											})
										}
										className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors ${
											settings.position === 'bottom'
												? 'bg-accent text-white'
												: 'bg-sidebar-line/50 text-ink-dull hover:bg-sidebar-line'
										}`}
									>
										Bottom
									</button>
									<button
										onClick={() =>
											onSettingsChange({
												...settings,
												position: 'top'
											})
										}
										className={`flex-1 rounded-md px-3 py-2 text-sm transition-colors ${
											settings.position === 'top'
												? 'bg-accent text-white'
												: 'bg-sidebar-line/50 text-ink-dull hover:bg-sidebar-line'
										}`}
									>
										Top
									</button>
								</div>
							</div>
						</div>
					</motion.div>
				</>
			)}
		</AnimatePresence>
	);
}
