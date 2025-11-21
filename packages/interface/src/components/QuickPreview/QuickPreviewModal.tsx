import { motion, AnimatePresence } from 'framer-motion';
import { X, ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useEffect } from 'react';
import type { File } from '@sd/ts-client';
import { useLibraryQuery } from '../../context';
import { Inspector } from '../../Inspector';
import { ContentRenderer } from './ContentRenderer';

interface QuickPreviewModalProps {
	fileId: string;
	isOpen: boolean;
	onClose: () => void;
	onNext?: () => void;
	onPrevious?: () => void;
	hasPrevious?: boolean;
	hasNext?: boolean;
}

export function QuickPreviewModal({
	fileId,
	isOpen,
	onClose,
	onNext,
	onPrevious,
	hasPrevious,
	hasNext
}: QuickPreviewModalProps) {
	const { data: file, isLoading, error } = useLibraryQuery(
		{
			type: 'files.by_id',
			input: { file_id: fileId }
		},
		{
			enabled: !!fileId && isOpen
		}
	);

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
				<>
					{/* Backdrop */}
					<motion.div
						key="backdrop"
						initial={{ opacity: 0 }}
						animate={{ opacity: 1 }}
						exit={{ opacity: 0 }}
						transition={{ duration: 0.15 }}
						className="fixed inset-0 z-[9999] bg-black/80 backdrop-blur-sm"
						onClick={onClose}
					/>

					{/* Modal - key stays constant so it doesn't remount on file change */}
					<motion.div
						key="modal"
						initial={{ opacity: 0, scale: 0.95 }}
						animate={{ opacity: 1, scale: 1 }}
						exit={{ opacity: 0, scale: 0.95 }}
						transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
						className="fixed inset-8 z-[9999] flex flex-col overflow-hidden rounded-2xl border border-app-line bg-app shadow-2xl"
						onClick={(e) => e.stopPropagation()}
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
								<div className="flex items-center justify-between border-b border-app-line bg-app-box/40 px-4 py-3 backdrop-blur-xl">
									<div className="flex flex-1 items-center gap-3">
										{/* Navigation Arrows */}
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

										<div className="h-4 w-px bg-app-line" />

										<div className="truncate text-sm font-medium">{file.name}</div>
									</div>

									<button
										onClick={onClose}
										className="rounded-md p-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
									>
										<X size={16} weight="bold" />
									</button>
								</div>

								{/* Content Area */}
								<div className="flex flex-1 overflow-hidden">
									{/* File Content */}
									<div className="flex-1 bg-app-box/30">
										<ContentRenderer file={file} />
									</div>

									{/* Inspector Sidebar */}
									<div className="w-[280px] min-w-[280px] overflow-hidden border-l border-app-line bg-app">
										<Inspector variant={{ type: "file", file }} showPopOutButton={false} />
									</div>
								</div>

								{/* Footer with keyboard hints */}
								<div className="border-t border-app-line bg-app-box/30 px-4 py-2">
									<div className="text-center text-xs text-ink-dull">
										<span className="text-ink">ESC</span> or{' '}
										<span className="text-ink">Space</span> to close
										{(hasPrevious || hasNext) && (
											<>
												{' • '}
												<span className="text-ink">←</span> /{' '}
												<span className="text-ink">→</span> to navigate
											</>
										)}
									</div>
								</div>
							</>
						)}
					</motion.div>
				</>
			)}
		</AnimatePresence>
	);
}
