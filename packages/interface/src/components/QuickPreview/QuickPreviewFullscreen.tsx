import { createPortal } from 'react-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { X, ArrowLeft, ArrowRight } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import type { File } from '@sd/ts-client';
import { useNormalizedCache } from '../../context';
import { ContentRenderer } from './ContentRenderer';
import { TopBarPortal } from '../../TopBar';

interface QuickPreviewFullscreenProps {
	fileId: string;
	isOpen: boolean;
	onClose: () => void;
	onNext?: () => void;
	onPrevious?: () => void;
	hasPrevious?: boolean;
	hasNext?: boolean;
	sidebarWidth?: number;
	inspectorWidth?: number;
}

const PREVIEW_LAYER_ID = 'quick-preview-layer';

export function QuickPreviewFullscreen({
	fileId,
	isOpen,
	onClose,
	onNext,
	onPrevious,
	hasPrevious,
	hasNext,
	sidebarWidth = 0,
	inspectorWidth = 0
}: QuickPreviewFullscreenProps) {
	const [portalTarget, setPortalTarget] = useState<HTMLElement | null>(null);
	const [isZoomed, setIsZoomed] = useState(false);

	// Reset zoom when file changes
	useEffect(() => {
		setIsZoomed(false);
	}, [fileId]);

	const { data: file, isLoading, error } = useNormalizedCache<{ file_id: string }, File>({
		wireMethod: 'query:files.by_id',
		input: { file_id: fileId },
		resourceType: 'file',
		resourceId: fileId,
		enabled: !!fileId && isOpen,
	});

	// Find portal target on mount
	useEffect(() => {
		const target = document.getElementById(PREVIEW_LAYER_ID);
		setPortalTarget(target);
	}, []);

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

	// Get background style based on content type
	const getBackgroundClass = () => {
		if (!file) return 'bg-black/90';

		switch (file.content_identity?.kind) {
			case 'video':
				return 'bg-black';
			case 'audio':
				return 'audio-gradient';
			case 'image':
				return 'bg-black/95';
			default:
				return 'bg-black/90';
		}
	};

	if (!portalTarget) return null;

	const content = (
		<AnimatePresence mode="wait">
			{isOpen && (
				<motion.div
					key="fullscreen-preview"
					initial={{ opacity: 0 }}
					animate={{ opacity: 1 }}
					exit={{ opacity: 0 }}
					transition={{ duration: 0.2 }}
					className={`absolute inset-0 flex flex-col ${getBackgroundClass()}`}
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
							{/* TopBar content via portal */}
							<TopBarPortal
								left={
									<div className="flex items-center gap-2">
										{(hasPrevious || hasNext) && (
											<>
												<button
													onClick={onPrevious}
													disabled={!hasPrevious}
													className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
												>
													<ArrowLeft size={16} weight="bold" />
												</button>
												<button
													onClick={onNext}
													disabled={!hasNext}
													className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-30"
												>
													<ArrowRight size={16} weight="bold" />
												</button>
												<div className="h-4 w-px bg-white/20 mx-1" />
											</>
										)}
									</div>
								}
								center={
									<div className="truncate text-sm font-medium text-white/90">
										{file.name}
									</div>
								}
								right={
									<button
										onClick={onClose}
										className="rounded-md p-1.5 text-white/70 transition-colors hover:bg-white/10 hover:text-white"
									>
										<X size={16} weight="bold" />
									</button>
								}
							/>

							{/* Content Area - padded to fit between sidebar/inspector, expands on zoom */}
							<div
								className={`flex-1 pt-14 pb-10 ${isZoomed ? 'overflow-visible' : 'overflow-hidden'}`}
								style={{
									paddingLeft: isZoomed ? 0 : sidebarWidth,
									paddingRight: isZoomed ? 0 : inspectorWidth,
								}}
							>
								<ContentRenderer file={file} onZoomChange={setIsZoomed} />
							</div>

							{/* Footer with keyboard hints */}
							<div className="absolute bottom-0 left-0 right-0 z-10 px-6 py-3">
								<div className="text-center text-xs text-white/50">
									<span className="text-white/70">ESC</span> or{' '}
									<span className="text-white/70">Space</span> to close
									{(hasPrevious || hasNext) && (
										<>
											{' · '}
											<span className="text-white/70">←</span> /{' '}
											<span className="text-white/70">→</span> to navigate
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

	return createPortal(content, portalTarget);
}

export { PREVIEW_LAYER_ID };
