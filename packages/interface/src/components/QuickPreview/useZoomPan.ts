import { useState, useCallback, useEffect, RefObject } from 'react';

interface UseZoomPanOptions {
	minZoom?: number;
	maxZoom?: number;
	zoomStep?: number;
}

export function useZoomPan(
	containerRef: RefObject<HTMLElement>,
	options: UseZoomPanOptions = {}
) {
	const { minZoom = 1, maxZoom = 5, zoomStep = 0.2 } = options;

	const [zoom, setZoom] = useState(1);
	const [pan, setPan] = useState({ x: 0, y: 0 });
	const [isDragging, setIsDragging] = useState(false);
	const [dragStart, setDragStart] = useState({ x: 0, y: 0 });

	// Reset zoom and pan
	const reset = useCallback(() => {
		setZoom(1);
		setPan({ x: 0, y: 0 });
	}, []);

	// Zoom in/out
	const zoomIn = useCallback(() => {
		setZoom((z) => Math.min(maxZoom, z + zoomStep));
	}, [maxZoom, zoomStep]);

	const zoomOut = useCallback(() => {
		setZoom((z) => {
			const newZoom = Math.max(minZoom, z - zoomStep);
			// Reset pan when zooming back to 1x
			if (newZoom === 1) {
				setPan({ x: 0, y: 0 });
			}
			return newZoom;
		});
	}, [minZoom, zoomStep]);

	// Mouse wheel zoom
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		const handleWheel = (e: WheelEvent) => {
			// Only zoom if not scrolling controls or other UI
			if ((e.target as HTMLElement).closest('input, button, [role="slider"]')) {
				return;
			}

			e.preventDefault();

			const delta = -e.deltaY;
			const zoomChange = delta > 0 ? zoomStep : -zoomStep;

			setZoom((z) => {
				const newZoom = Math.max(minZoom, Math.min(maxZoom, z + zoomChange));
				// Reset pan when zooming back to 1x
				if (newZoom === 1) {
					setPan({ x: 0, y: 0 });
				}
				return newZoom;
			});
		};

		container.addEventListener('wheel', handleWheel, { passive: false });
		return () => container.removeEventListener('wheel', handleWheel);
	}, [containerRef, minZoom, maxZoom, zoomStep]);

	// Pan with mouse drag (only when zoomed in)
	useEffect(() => {
		const container = containerRef.current;
		if (!container || zoom <= 1) return;

		const handleMouseDown = (e: MouseEvent) => {
			// Don't pan if clicking on controls
			if ((e.target as HTMLElement).closest('button, input, [role="slider"]')) {
				return;
			}

			setIsDragging(true);
			setDragStart({ x: e.clientX - pan.x, y: e.clientY - pan.y });
			container.style.cursor = 'grabbing';
		};

		const handleMouseMove = (e: MouseEvent) => {
			if (!isDragging) return;

			setPan({
				x: e.clientX - dragStart.x,
				y: e.clientY - dragStart.y
			});
		};

		const handleMouseUp = () => {
			setIsDragging(false);
			if (zoom > 1) {
				container.style.cursor = 'grab';
			} else {
				container.style.cursor = 'default';
			}
		};

		container.addEventListener('mousedown', handleMouseDown);
		window.addEventListener('mousemove', handleMouseMove);
		window.addEventListener('mouseup', handleMouseUp);

		// Set cursor
		container.style.cursor = zoom > 1 ? 'grab' : 'default';

		return () => {
			container.removeEventListener('mousedown', handleMouseDown);
			window.removeEventListener('mousemove', handleMouseMove);
			window.removeEventListener('mouseup', handleMouseUp);
			container.style.cursor = 'default';
		};
	}, [containerRef, zoom, pan, isDragging, dragStart]);

	// Keyboard shortcuts
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			// Don't interfere with inputs
			if ((e.target as HTMLElement).tagName === 'INPUT') {
				return;
			}

			if (e.key === '=' || e.key === '+') {
				e.preventDefault();
				zoomIn();
			} else if (e.key === '-' || e.key === '_') {
				e.preventDefault();
				zoomOut();
			} else if (e.key === '0') {
				e.preventDefault();
				reset();
			}
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	}, [zoomIn, zoomOut, reset]);

	return {
		zoom,
		pan,
		zoomIn,
		zoomOut,
		reset,
		transform: {
			transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
			transition: isDragging ? 'none' : 'transform 0.1s ease-out'
		}
	};
}
