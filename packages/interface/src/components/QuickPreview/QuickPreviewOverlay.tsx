import { motion, AnimatePresence } from 'framer-motion';
import { X, ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useEffect } from 'react';
import type { File } from '@sd/ts-client';
import { useNormalizedQuery } from '../../contexts/SpacedriveContext';
import { ContentRenderer } from './ContentRenderer';

interface QuickPreviewOverlayProps {
	fileId: string;
	isOpen: boolean;
	onClose: () => void;
	onNext?: () => void;
	onPrevious?: () => void;
	hasPrevious?: boolean;
	hasNext?: boolean;
}

export function QuickPreviewOverlay({
	fileId,
	isOpen,
	onClose,
	onNext,
	onPrevious,
	hasPrevious,
	hasNext
}: QuickPreviewOverlayProps) {
	const { data: file, isLoading, error } = useNormalizedQuery<{ file_id: string }, File>({
		wireMethod: 'query:files.by_id',
		input: { file_id: fileId },
		resourceType: 'file',
		resourceId: fileId,
		enabled: !!fileId && isOpen,
	});

	useEffect(() => {
		if (!isOpen) return;

		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.code === 'Escape' || e.code === 'Space') {
				e.preventDefault();
				onClose();
			}
			if (e.code === 'ArrowLeft' && hasPrevious && onPrevious) {
				e.preventDefault();
				onPrevious();
			}
			if (e.code === 'ArrowRight' && hasNext && onNext) {
				e.preventDefault();
				onNext();
			}
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [isOpen, onClose, onNext, onPrevious, hasPrevious, hasNext]);

	return (
		<AnimatePresence mode="wait">
			{isOpen && (
				<motion.div
					key="overlay"
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{ duration: 0.15 }}
					className="absolute inset-0 z-50 flex flex-col overflow-hidden rounded-lg bg-app/95 backdrop-blur-xl"
				>
					{isLoading || !file ? (
						<div className="flex h-full items-center justify-center text-ink">
							<div className="animate-pulse">Loading...</div>
						</div>
					) : error ? (
						<div className="flex h-full items-center justify-center text-red-400">
							<div>
								<div className="mb-2 text-lg font-medium">Error loading file</div>
								<div className="text-sm">{error.message}</div>
							</div>
						</div>
					) : (
						<>
							{/* Header */}
							<div className="flex items-center justify-between border-b border-app-line/50 bg-app-box/40 px-4 py-2">
								<div className="flex flex-1 items-center gap-3">
									{/* Navigation Arrows */}
									{(hasPrevious || hasNext) && (
										<>
											<div className="flex items-center gap-1">
												<button
													onClick={onPrevious}
													disabled={!hasPrevious}
													className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink disabled:opacity-30"
												>
													<ArrowLeft size={16} weight="bold" />
												</button>
												<button
													onClick={onNext}
													disabled={!hasNext}
													className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink disabled:opacity-30"
												>
													<ArrowRight size={16} weight="bold" />
												</button>
											</div>
											<div className="h-4 w-px bg-app-line/50" />
										</>
									)}
									<div className="truncate text-sm font-medium text-ink">{file.name}</div>
								</div>

								<button
									onClick={onClose}
									className="rounded-md p-1.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
								>
									<X size={16} weight="bold" />
								</button>
							</div>

							{/* Content Area - full width, no inspector */}
							<div className="flex-1 overflow-hidden">
								<ContentRenderer file={file} />
							</div>

							{/* Footer with keyboard hints */}
							<div className="border-t border-app-line/50 bg-app-box/40 px-4 py-1.5">
								<div className="text-center text-xs text-ink-dull">
									<span className="text-ink">ESC</span> or{' '}
									<span className="text-ink">Space</span> to close
									{(hasPrevious || hasNext) && (
										<>
											{' · '}
											<span className="text-ink">←</span> /{' '}
											<span className="text-ink">→</span> to navigate
										</>
									)}
								</div>
							</div>
						</>
					)}
				</motion.div>
			)}
		</AnimatePresence>
	);
}