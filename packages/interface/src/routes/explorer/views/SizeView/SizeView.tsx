import {
	ArrowCounterClockwise,
	ArrowsOut,
	Minus,
	Plus
} from '@phosphor-icons/react';
import type {DirectorySortBy, File} from '@sd/ts-client';
import {TopBarButton, TopBarButtonGroup} from '@sd/ui';
import * as d3 from 'd3';
import {useEffect, useMemo, useRef, useState} from 'react';
import {createPortal} from 'react-dom';
import {useTabManager} from '../../../../components/TabManager/useTabManager';
import {ServerContext, useServer} from '../../../../contexts/ServerContext';
import {useNormalizedQuery} from '../../../../contexts/SpacedriveContext';
import {useExplorer} from '../../context';
import {Thumb} from '../../File/Thumb';
import {useFileContextMenu} from '../../hooks/useFileContextMenu';
import {useDraggableFile} from '../../hooks/useDraggableFile';
import {useSelection} from '../../SelectionContext';
import {formatBytes} from '../../utils';

// Cache for computed colors
const colorCache = new Map<string, string>();

// Gradient ID for folder bubbles
const FOLDER_GRADIENT_ID = 'folder-accent-gradient';

// Portal layer for fullscreen size view
const SIZE_VIEW_LAYER_ID = 'size-view-layer';

// Get computed color from Tailwind class
function getTailwindColor(className: string): string {
	if (colorCache.has(className)) {
		return colorCache.get(className)!;
	}

	const div = document.createElement('div');
	div.className = className;
	div.style.display = 'none';
	document.body.appendChild(div);
	const color = getComputedStyle(div).backgroundColor;
	document.body.removeChild(div);

	colorCache.set(className, color);
	return color;
}

// Get accent colors for gradient from CSS custom properties
function getAccentColors(): {faint: string; base: string; deep: string} {
	const root = document.documentElement;
	const style = getComputedStyle(root);

	// CSS variables store HSL values like "208, 100%, 57%"
	const accentFaint = style.getPropertyValue('--color-accent-faint').trim();
	const accent = style.getPropertyValue('--color-accent').trim();
	const accentDeep = style.getPropertyValue('--color-accent-deep').trim();

	return {
		faint: accentFaint ? `hsl(${accentFaint})` : 'hsl(208, 100%, 64%)',
		base: accent ? `hsl(${accent})` : 'hsl(208, 100%, 57%)',
		deep: accentDeep ? `hsl(${accentDeep})` : 'hsl(208, 100%, 47%)'
	};
}

function getFileColorClass(file: File): string {
	if (file.kind === 'Directory') return 'bg-accent'; // Used for selection stroke

	const ext = file.name.split('.').pop()?.toLowerCase() || '';

	// Images - lighter app-box
	if (['jpg', 'jpeg', 'png', 'gif', 'svg', 'webp', 'heic'].includes(ext)) {
		return 'bg-app-light-box';
	}

	// Videos - app-selected
	if (['mp4', 'mov', 'avi', 'mkv', 'webm'].includes(ext)) {
		return 'bg-app-selected';
	}

	// Audio - app-hover
	if (['mp3', 'wav', 'flac', 'aac', 'ogg'].includes(ext)) {
		return 'bg-app-hover';
	}

	// Documents - app-active
	if (['pdf', 'doc', 'docx', 'txt', 'md'].includes(ext)) {
		return 'bg-app-active';
	}

	// Code - app-input
	if (
		['js', 'ts', 'jsx', 'tsx', 'py', 'rs', 'go', 'java', 'cpp'].includes(
			ext
		)
	) {
		return 'bg-app-input';
	}

	// Archives - app-button
	if (['zip', 'tar', 'gz', 'rar', '7z'].includes(ext)) {
		return 'bg-app-button';
	}

	return 'bg-app-box';
}

function getFileColor(file: File): string {
	// Directories use the gradient
	if (file.kind === 'Directory') {
		return `url(#${FOLDER_GRADIENT_ID})`;
	}
	return getTailwindColor(getFileColorClass(file));
}

function getFileType(file: File): string {
	if (file.kind === 'Directory') return 'Folder';
	if (file.extension) return file.extension.toUpperCase();
	return 'File';
}

// Thumb overlay component with drag support
interface ThumbOverlayProps {
	overlay: {
		id: string;
		file: File;
		screenX: number;
		screenY: number;
		size: number;
	};
	selectedFiles: File[];
	selectFileRef: React.MutableRefObject<any>;
	navigateToPathRef: React.MutableRefObject<any>;
	filesRef: React.MutableRefObject<File[]>;
	contextMenuRef: React.MutableRefObject<any>;
	setContextMenuFile: (file: File) => void;
	clickTimeoutRef: React.MutableRefObject<NodeJS.Timeout | null>;
	svgRef: React.RefObject<SVGSVGElement>;
	zoomBehaviorRef: React.MutableRefObject<d3.ZoomBehavior<SVGSVGElement, unknown> | null>;
	gRef: React.MutableRefObject<d3.Selection<SVGGElement, unknown, null, undefined> | null>;
}

function ThumbOverlay({
	overlay,
	selectedFiles,
	selectFileRef,
	navigateToPathRef,
	filesRef,
	contextMenuRef,
	setContextMenuFile,
	clickTimeoutRef,
	svgRef,
	zoomBehaviorRef,
	gRef,
}: ThumbOverlayProps) {
	const selected = selectedFiles.some((f) => f.id === overlay.file.id);

	const {
		attributes,
		listeners,
		setNodeRef,
		isDragging,
	} = useDraggableFile({
		file: overlay.file,
		selectedFiles: selected && selectedFiles.length > 0 ? selectedFiles : undefined,
		// Pass the actual thumb size as gridSize so DragOverlay renders it at the right size
		// DragOverlay multiplies by 0.6, so we reverse it
		gridSize: overlay.size / 0.6,
	});

	return (
		<div
			ref={setNodeRef}
			{...listeners}
			{...attributes}
			key={overlay.id}
			className="pointer-events-auto absolute cursor-pointer overflow-hidden rounded-lg"
			style={{
				// Adjust left/top to center the thumb instead of using transform
				// This prevents drag offset issues with @dnd-kit
				left: overlay.screenX - (overlay.size / 2),
				top: overlay.screenY - (overlay.size * 0.6),
				width: overlay.size,
				height: overlay.size,
				opacity: isDragging ? 0.4 : 1,
			}}
			onWheel={(event) => {
				// Forward wheel events to the SVG to allow pinch-to-zoom over thumbnails
				if (svgRef.current) {
					const wheelEvent = new WheelEvent(event.type, event.nativeEvent);
					svgRef.current.dispatchEvent(wheelEvent);
				}
			}}
			onClick={(event) => {
				event.stopPropagation();

				// In size view, treat shift as multi-select (not range)
				// Range selection doesn't make sense with circular bubble positioning
				const multi = event.metaKey || event.ctrlKey || event.shiftKey;
				const range = false;

				selectFileRef.current(
					overlay.file,
					filesRef.current,
					multi,
					range
				);

				// Clear any existing zoom timeout
				if (clickTimeoutRef.current) {
					clearTimeout(clickTimeoutRef.current);
					clickTimeoutRef.current = null;
				}

				// Delay zoom-to-focus to allow double-click detection
				if (!multi && !range && svgRef.current && zoomBehaviorRef.current) {
					// Find the bubble data for this overlay
					const bubbleNode = gRef.current?.selectAll<SVGGElement, any>('g.bubble-node')
						.filter((d: any) => d.data.id === overlay.id);

					if (bubbleNode && !bubbleNode.empty()) {
						const d = bubbleNode.datum();

						clickTimeoutRef.current = setTimeout(() => {
							if (!svgRef.current || !zoomBehaviorRef.current) return;

							const svgElement = svgRef.current;
							const width = svgElement.clientWidth;
							const height = svgElement.clientHeight;
							const centerX = width / 2;
							const centerY = height / 2;

							const targetBubbleScreenSize = Math.min(width, height) * 0.4;
							const bubbleSize = d.r * 2;
							const targetScale = targetBubbleScreenSize / bubbleSize;

							const newTransform = d3.zoomIdentity
								.translate(centerX, centerY)
								.scale(targetScale)
								.translate(-d.x, -d.y);

							d3.select(svgElement)
								.transition()
								.duration(400)
								.call(zoomBehaviorRef.current!.transform, newTransform);
						}, 200);
					}
				}
			}}
			onDoubleClick={(event) => {
				event.stopPropagation();

				// Clear single click timeout
				if (clickTimeoutRef.current) {
					clearTimeout(clickTimeoutRef.current);
					clickTimeoutRef.current = null;
				}

				// Navigate if directory
				if (overlay.file.kind === 'Directory') {
					navigateToPathRef.current(overlay.file.sd_path);
				}
			}}
			onContextMenu={async (event) => {
				event.preventDefault();
				event.stopPropagation();

				// Select the file if not already selected
				const isSelected = selectedFiles.some(
					(f) => f.id === overlay.file.id
				);
				if (!isSelected) {
					selectFileRef.current(
						overlay.file,
						filesRef.current,
						false,
						false
					);
				}

				// Set the context menu file and show menu
				setContextMenuFile(overlay.file);

				// Show context menu on next tick after state updates
				setTimeout(async () => {
					await contextMenuRef.current.show(event);
				}, 0);
			}}
		>
			<Thumb
				file={overlay.file}
				size={overlay.size}
				className="drop-shadow-lg"
				frameClassName="border-0 bg-transparent rounded-lg"
				iconScale={0.7}
			/>
		</div>
	);
}

export function SizeView() {
	const {
		currentPath,
		sortBy,
		navigateToPath,
		viewSettings,
		sidebarVisible,
		inspectorVisible,
		activeTabId,
		sizeViewTransform,
		setSizeViewTransform,
		viewMode,
		setCurrentFiles
	} = useExplorer();

	const { tabs } = useTabManager();


	const {selectedFiles, selectFile} = useSelection();
	const serverContext = useServer();

	// Calculate sidebar and inspector widths
	const sidebarWidth = sidebarVisible ? 220 : 0;
	const inspectorWidth = inspectorVisible ? 280 : 0;

	// Find portal target (re-lookup when tab changes to ensure it's always found)
	const portalTarget = useMemo(
		() => document.getElementById(SIZE_VIEW_LAYER_ID),
		[activeTabId]
	);

	// Track which path+tab the current data belongs to
	const [dataSource, setDataSource] = useState<{
		tabId: string;
		path: SdPath | null;
	} | null>(null);

	const directoryQuery = useNormalizedQuery({
		wireMethod: 'query:files.directory_listing',
		input: currentPath
			? {
					path: currentPath,
					limit: null,
					include_hidden: false,
					sort_by: sortBy as DirectorySortBy,
					folders_first: viewSettings.foldersFirst
				}
			: null!,
		resourceType: 'file',
		enabled: !!currentPath,
		pathScope: currentPath ?? undefined,
		queryKey: ['directory_listing', 'size_view', activeTabId, currentPath]
	});

	// Update data source when query succeeds
	useEffect(() => {
		if (directoryQuery.isSuccess && currentPath) {
			setDataSource({tabId: activeTabId, path: currentPath});
		}
	}, [directoryQuery.isSuccess, activeTabId, currentPath]);

	// Only show files if they match the current tab and path
	const files = useMemo(() => {
		if (!directoryQuery.data?.files) return [];

		// Check if data source matches current context
		const currentSource = JSON.stringify({
			tabId: activeTabId,
			path: currentPath
		});
		const loadedSource = JSON.stringify(dataSource);

		if (currentSource !== loadedSource) {
			// Data doesn't match current tab/path, don't show it
			return [];
		}

		return directoryQuery.data.files;
	}, [directoryQuery.data, activeTabId, currentPath, dataSource]);

	// Update explorer context with raw file count (not filtered)
	useEffect(() => {
		setCurrentFiles(directoryQuery.data?.files || []);
	}, [directoryQuery.data?.files, setCurrentFiles]);

	const svgRef = useRef<SVGSVGElement>(null);
	const zoomBehaviorRef = useRef<d3.ZoomBehavior<
		SVGSVGElement,
		unknown
	> | null>(null);
	const clickTimeoutRef = useRef<NodeJS.Timeout | null>(null);
	const lastContextRef = useRef<{tabId: string, path: SdPath | null} | null>(null);
	const lastAppliedZoomRef = useRef<{tabId: string, zoom: number} | null>(null);
	// Track which tab we just switched to (allows one path update from TabNavigationSync)
	const justSwitchedToTabRef = useRef<string | null>(null);
	const [contextMenuFile, setContextMenuFile] = useState<File | null>(null);
	const [thumbOverlays, setThumbOverlays] = useState<
		Array<{
			id: string;
			file: File;
			screenX: number;
			screenY: number;
			size: number;
		}>
	>([]);

	// Create context menu for the current file
	const contextMenu = useFileContextMenu({
		file: contextMenuFile || files[0],
		selectedFiles,
		selected: contextMenuFile
			? selectedFiles.some((f) => f.id === contextMenuFile.id)
			: false
	});

	// Use refs for stable function references
	const selectFileRef = useRef(selectFile);
	const navigateToPathRef = useRef(navigateToPath);
	const filesRef = useRef(files);
	const selectedFilesRef = useRef(selectedFiles);
	const gRef = useRef<d3.Selection<
		SVGGElement,
		unknown,
		null,
		undefined
	> | null>(null);
	const contextMenuRef = useRef(contextMenu);

	useEffect(() => {
		selectFileRef.current = selectFile;
		navigateToPathRef.current = navigateToPath;
		filesRef.current = files;
		selectedFilesRef.current = selectedFiles;
		contextMenuRef.current = contextMenu;
	}, [selectFile, navigateToPath, files, selectedFiles, contextMenu]);

	// Function to update thumb overlay positions based on current transform
	const updateThumbOverlays = (transform: d3.ZoomTransform) => {
		if (!svgRef.current || !gRef.current) return;

		const svgRect = svgRef.current.getBoundingClientRect();

		const overlays: Array<{
			id: string;
			file: File;
			screenX: number;
			screenY: number;
			size: number;
		}> = [];

		gRef.current
			.selectAll<SVGGElement, any>('g.bubble-node')
			.each(function (d: any) {
				const screenRadius = d.r * transform.k;

				// Show thumbnails when effective screen radius > 40px
				if (screenRadius > 40) {
					// Convert SVG coordinates to absolute screen coordinates
					const screenX =
						d.x * transform.k + transform.x + svgRect.left;
					const screenY =
						d.y * transform.k + transform.y + svgRect.top;

					overlays.push({
						id: d.data.id,
						file: d.data.file,
						screenX,
						screenY,
						size: Math.min(screenRadius * 0.9, 400)
					});
				}
			});

		setThumbOverlays(overlays);
	};

	// Initialize zoom behavior once (after portal is ready)
	useEffect(() => {
		if (!svgRef.current || !portalTarget) return;

		const svg = d3.select(svgRef.current);

		// Only create g element if it doesn't exist
		let g = gRef.current;
		if (!g || g.empty()) {
			svg.selectAll('*').remove();

			// Add gradient definition for folder bubbles
			const defs = svg.append('defs');
			const accentColors = getAccentColors();

			const gradient = defs
				.append('radialGradient')
				.attr('id', FOLDER_GRADIENT_ID)
				.attr('cx', '30%')
				.attr('cy', '30%')
				.attr('r', '70%');

			// Highlight at top-left for 3D effect
			gradient
				.append('stop')
				.attr('offset', '0%')
				.attr('stop-color', accentColors.faint);

			gradient
				.append('stop')
				.attr('offset', '50%')
				.attr('stop-color', accentColors.base);

			gradient
				.append('stop')
				.attr('offset', '100%')
				.attr('stop-color', accentColors.deep);

			g = svg.append('g');
			gRef.current = g;
		}

		const updateTextOnZoom = (scale: number) => {
			// Update text transform for constant size
			g.selectAll<SVGTextElement, any>('text').attr(
				'transform',
				`scale(${1 / scale})`
			);

			// Update text content based on effective radius
			g.selectAll<SVGGElement, any>('g.bubble-node').each(function (
				d: any
			) {
				const node = d3.select(this);
				const textElement = node.select('text');
				const effectiveRadius = d.r * scale;

				textElement.selectAll('tspan').remove();

				if (effectiveRadius < 25) return;

				// For large circles with Thumb, position text at bottom
				const hasThumb = effectiveRadius > 40;
				const baseY = hasThumb
					? effectiveRadius * 0.55
					: effectiveRadius > 40
						? -10
						: 0;

				const nameTspan = textElement
					.append('tspan')
					.attr('x', 0)
					.attr('y', baseY);

				if (effectiveRadius > 80) {
					nameTspan.attr('font-size', '11px');
				} else if (effectiveRadius > 50) {
					nameTspan.attr('font-size', '10px');
				} else {
					nameTspan.attr('font-size', '9px');
				}

				const maxLength = Math.floor(effectiveRadius / 5);
				nameTspan.text(
					d.data.name.length > maxLength
						? d.data.name.slice(0, maxLength) + '...'
						: d.data.name
				);

				if (effectiveRadius > 40) {
					textElement
						.append('tspan')
						.attr('x', 0)
						.attr('y', baseY + 14)
						.attr(
							'font-size',
							effectiveRadius > 80 ? '11px' : '10px'
						)
						.attr('font-weight', '700')
						.text(formatBytes(d.data.value));
				}
			});
		};

		const updateStrokeWidthsForZoom = (scale: number) => {
			// Only adjust stroke width for zoom, don't change selection state
			const baseStrokeWidth = 4;

			svg.selectAll<SVGCircleElement, any>('circle[data-file-id]').each(function() {
				const circle = d3.select(this);
				const currentStroke = circle.attr('stroke');

				// If this circle has a stroke (is selected), update its width
				if (currentStroke && currentStroke !== 'transparent') {
					circle.attr('stroke-width', baseStrokeWidth / scale);
				}
			});
		};

		const zoom = d3
			.zoom<SVGSVGElement, unknown>()
			.scaleExtent([0.1, 100])
			.on('zoom', (event) => {
				g.attr('transform', event.transform);
				setSizeViewTransform({
					k: event.transform.k,
					x: event.transform.x,
					y: event.transform.y,
				});
				updateTextOnZoom(event.transform.k);
				updateStrokeWidthsForZoom(event.transform.k);
				updateThumbOverlays(event.transform);
			});

		svg.call(zoom);
		zoomBehaviorRef.current = zoom;

		// Double-click to reset zoom
		svg.on('dblclick.zoom', () => {
			svg.transition()
				.duration(300)
				.call(zoom.transform, d3.zoomIdentity)
				.on('end', () => {
					setSizeViewTransform({ k: 1, x: 0, y: 0 });
					updateTextOnZoom(1);
				});
		});

		return () => {
			svg.selectAll('*').remove();
			gRef.current = null;
			if (clickTimeoutRef.current) {
				clearTimeout(clickTimeoutRef.current);
			}
		};
	}, [portalTarget]); // Run when portal is ready

	// Reset zoom when path changes within the same tab (but not on tab switch or initial mount)
	useEffect(() => {
		const lastContext = lastContextRef.current;
		const currentContext = { tabId: activeTabId, path: currentPath };

		// Detect tab switch or initial mount
		const tabIdChanged = lastContext && lastContext.tabId !== currentContext.tabId;
		const justMounted = !lastContext;

		// When we switch tabs or mount, mark this tab as "expecting path sync"
		if (tabIdChanged || justMounted) {
			justSwitchedToTabRef.current = currentContext.tabId;
		}

		// Check if path changed
		const pathChanged = lastContext && JSON.stringify(lastContext.path) !== JSON.stringify(currentContext.path);

		// If we're still in the tab we just switched to, this is TabNavigationSync catching up
		const isPathSyncingAfterTabSwitch = justSwitchedToTabRef.current === currentContext.tabId;

		// Reset zoom if path changed AND we're not syncing after tab switch
		if (pathChanged && !isPathSyncingAfterTabSwitch) {
			if (svgRef.current && zoomBehaviorRef.current) {
				const svg = d3.select(svgRef.current);
				svg.call(zoomBehaviorRef.current.transform, d3.zoomIdentity);
				setSizeViewTransform({ k: 1, x: 0, y: 0 });
			}
		}

		// If path changed while we were syncing, clear the flag (next change will reset)
		if (pathChanged && isPathSyncingAfterTabSwitch) {
			justSwitchedToTabRef.current = null;
		}

		// Update last context for next comparison
		lastContextRef.current = currentContext;
	}, [currentPath, activeTabId, setSizeViewTransform]);

	const bubbleData = useMemo(() => {
		const itemLimit = Math.min(viewSettings.sizeViewItemLimit || 500, files.length);

		// Separate folders and files
		const folders = files.filter((f) => f.kind === 'Directory');
		const regularFiles = files.filter((f) => f.kind !== 'Directory' && f.size > 0);

		// Always include all folders, then fill with largest files
		const sortedFiles = regularFiles.sort((a, b) => b.size - a.size);
		const topFiles = sortedFiles.slice(0, Math.max(0, itemLimit - folders.length));

		const combined = [...folders, ...topFiles];

		if (combined.length === 0) return [];

		// Calculate average size of files to give folders a reasonable minimum
		const averageFileSize = topFiles.length > 0
			? topFiles.reduce((sum, f) => sum + f.size, 0) / topFiles.length
			: 1000000; // 1MB default

		// Give folders a minimum size of 2.5% of average file size (so they're visible)
		const minFolderSize = averageFileSize * 0.025;

		const mapped = combined.map((file) => ({
			id: file.id,
			name: file.name,
			value: file.kind === 'Directory'
				? Math.max(file.size, minFolderSize)
				: file.size,
			file,
			color: getFileColor(file),
			type: getFileType(file)
		}));

		return mapped;
	}, [files, viewSettings.sizeViewItemLimit]);

	// Update chart data (preserves zoom state)
	useEffect(() => {
		if (!svgRef.current || !gRef.current) return;

		const g = gRef.current;
		const width = svgRef.current.clientWidth;
		const height = svgRef.current.clientHeight;

		// Clear bubbles if no data or no dimensions
		if (bubbleData.length === 0 || width === 0 || height === 0) {
			g.selectAll('g.bubble-node').remove();
			setThumbOverlays([]);
			return;
		}

		const pack = d3.pack().size([width, height]).padding(3);

		const root = pack(
			d3.hierarchy({children: bubbleData}).sum((d) => d.value)
		);

		// Update nodes with data join (preserves existing nodes when possible)
		const nodes = g
			.selectAll<SVGGElement, any>('g.bubble-node')
			.data(root.leaves(), (d: any) => d.data.id)
			.join(
				(enter) =>
					enter
						.append('g')
						.attr('class', 'bubble-node')
						.attr('transform', (d) => `translate(${d.x},${d.y})`)
						.style('cursor', 'pointer'),
				(update) =>
					update.attr('transform', (d) => `translate(${d.x},${d.y})`),
				(exit) => exit.remove()
			);

		// Update or create circles (background for Thumb or standalone for small circles)
		nodes
			.selectAll<SVGCircleElement, any>('circle')
			.data((d) => [d])
			.join('circle')
			.attr('r', (d) => d.r)
			.attr('fill', (d) => d.data.color)
			.attr('fill-opacity', 0.4)
			.attr('stroke', 'transparent')
			.attr('stroke-width', 0)
			.attr('data-file-id', (d) => d.data.id)
			.on('click', (event, d) => {
				event.stopPropagation();

				// In size view, treat shift as multi-select (not range)
				// Range selection doesn't make sense with circular bubble positioning
				const multi = event.metaKey || event.ctrlKey || event.shiftKey;
				const range = false;

				// Select immediately for responsive feedback
				selectFileRef.current(
					d.data.file,
					filesRef.current,
					multi,
					range
				);

				// Clear any existing zoom timeout
				if (clickTimeoutRef.current) {
					clearTimeout(clickTimeoutRef.current);
					clickTimeoutRef.current = null;
				}

				// Delay zoom-to-focus to allow double-click detection
				if (
					!multi &&
					!range &&
					svgRef.current &&
					zoomBehaviorRef.current
				) {
					clickTimeoutRef.current = setTimeout(() => {
						if (!svgRef.current || !zoomBehaviorRef.current) return;

						const svgElement = svgRef.current;
						const width = svgElement.clientWidth;
						const height = svgElement.clientHeight;
						const centerX = width / 2;
						const centerY = height / 2;

						// Target: make the bubble appear at a consistent size on screen
						const targetBubbleScreenSize =
							Math.min(width, height) * 0.4;
						const bubbleSize = d.r * 2;
						const targetScale = targetBubbleScreenSize / bubbleSize;

						const newTransform = d3.zoomIdentity
							.translate(centerX, centerY)
							.scale(targetScale)
							.translate(-d.x, -d.y);

						d3.select(svgElement)
							.transition()
							.duration(400)
							.call(
								zoomBehaviorRef.current!.transform,
								newTransform
							);
					}, 200);
				}
			})
			.on('dblclick', (event, d) => {
				event.stopPropagation();

				// Clear single click timeout
				if (clickTimeoutRef.current) {
					clearTimeout(clickTimeoutRef.current);
					clickTimeoutRef.current = null;
				}

				// Navigate if directory
				if (d.data.file.kind === 'Directory') {
					navigateToPathRef.current(d.data.file.sd_path);
				}
			})
			.on('contextmenu', async (event, d) => {
				event.preventDefault();
				event.stopPropagation();

				// Select the file if not already selected
				const isSelected = selectedFiles.some(
					(f) => f.id === d.data.file.id
				);
				if (!isSelected) {
					selectFileRef.current(
						d.data.file,
						filesRef.current,
						false,
						false
					);
				}

				// Set the context menu file and show menu
				setContextMenuFile(d.data.file);

				// Show context menu on next tick after state updates
				setTimeout(async () => {
					await contextMenuRef.current.show(event);
				}, 0);
			})
			.on('mouseenter', function (event, d) {
				d3.select(this)
					.transition()
					.duration(150)
					.attr('filter', 'brightness(1.15)');
			})
			.on('mouseleave', function (event, d) {
				d3.select(this).transition().duration(150).attr('filter', null);
			});

		// Update or create titles
		nodes
			.selectAll<SVGTitleElement, any>('title')
			.data((d) => [d])
			.join('title')
			.text((d) => `${d.data.name}\n${formatBytes(d.data.value)}`);

		// Update thumb overlays after bubbles are rendered
		if (svgRef.current) {
			const currentTransform = d3.zoomTransform(svgRef.current);
			updateThumbOverlays(currentTransform);
		}

		// Update or create text elements
		nodes
			.selectAll<SVGTextElement, any>('text')
			.data((d) => [d])
			.join('text')
			.attr('text-anchor', 'middle')
			.attr('fill', 'white')
			.attr('font-weight', '600')
			.style('pointer-events', 'none');

		// Trigger text update with current zoom level
		if (svgRef.current) {
			const currentTransform = d3.zoomTransform(svgRef.current);
			const scale = currentTransform.k;

			// Update text transform and content
			g.selectAll<SVGTextElement, any>('text').attr(
				'transform',
				`scale(${1 / scale})`
			);

			nodes.each(function (d) {
				const node = d3.select(this);
				const textElement = node.select('text');
				const effectiveRadius = d.r * scale;

				textElement.selectAll('tspan').remove();

				if (effectiveRadius < 25) return;

				// For large circles with Thumb, position text at bottom
				const hasThumb = effectiveRadius > 40;
				const baseY = hasThumb
					? effectiveRadius * 0.55
					: effectiveRadius > 40
						? -10
						: 0;

				const nameTspan = textElement
					.append('tspan')
					.attr('x', 0)
					.attr('y', baseY);

				if (effectiveRadius > 80) {
					nameTspan.attr('font-size', '11px');
				} else if (effectiveRadius > 50) {
					nameTspan.attr('font-size', '10px');
				} else {
					nameTspan.attr('font-size', '9px');
				}

				const maxLength = Math.floor(effectiveRadius / 5);
				nameTspan.text(
					d.data.name.length > maxLength
						? d.data.name.slice(0, maxLength) + '...'
						: d.data.name
				);

				if (effectiveRadius > 40) {
					textElement
						.append('tspan')
						.attr('x', 0)
						.attr('y', baseY + 14)
						.attr(
							'font-size',
							effectiveRadius > 80 ? '11px' : '10px'
						)
						.attr('font-weight', '700')
						.text(formatBytes(d.data.value));
				}
			});
		}
	}, [bubbleData]);

	// Apply stored transform when tab changes or bubbles first render
	useEffect(() => {
		if (!svgRef.current || !zoomBehaviorRef.current || bubbleData.length === 0) return;

		// Check if we've already applied transform for this tab
		if (lastAppliedZoomRef.current?.tabId === activeTabId) {
			return;
		}
		const svg = d3.select(svgRef.current);
		const transform = d3.zoomIdentity
			.translate(sizeViewTransform.x, sizeViewTransform.y)
			.scale(sizeViewTransform.k);
		svg.call(zoomBehaviorRef.current.transform, transform);

		// Mark this tab as having transform applied
		lastAppliedZoomRef.current = { tabId: activeTabId, zoom: sizeViewTransform.k };
	}, [activeTabId, bubbleData.length, sizeViewTransform]);

	// Update selection strokes when selectedFiles changes or bubbles are rendered
	useEffect(() => {
		if (!svgRef.current) return;

		const svg = d3.select(svgRef.current);
		const accentColor = getTailwindColor('bg-accent');
		const currentTransform = d3.zoomTransform(svgRef.current);
		const scale = currentTransform.k;
		const baseStrokeWidth = 4;

		// Update both stroke color and width based on selection and current zoom
		svg.selectAll<SVGCircleElement, any>('circle[data-file-id]')
			.attr('stroke', (d) => {
				const isSelected = selectedFiles.some(
					(f) => f.id === d.data.id
				);
				return isSelected ? accentColor : 'transparent';
			})
			.attr('stroke-width', (d) => {
				const isSelected = selectedFiles.some(
					(f) => f.id === d.data.id
				);
				return isSelected ? baseStrokeWidth / scale : 0;
			});
	}, [selectedFiles, bubbleData.length]);

	const handleResetZoom = () => {
		if (!svgRef.current || !zoomBehaviorRef.current) return;
		const svg = d3.select(svgRef.current);
		svg.transition()
			.duration(300)
			.call(zoomBehaviorRef.current.transform, d3.zoomIdentity)
			.on('end', () => setSizeViewTransform({ k: 1, x: 0, y: 0 }));
	};

	const handleZoomIn = () => {
		if (!svgRef.current || !zoomBehaviorRef.current) return;
		const svg = d3.select(svgRef.current);
		svg.transition()
			.duration(200)
			.call(zoomBehaviorRef.current.scaleBy, 1.3);
	};

	const handleZoomOut = () => {
		if (!svgRef.current || !zoomBehaviorRef.current) return;
		const svg = d3.select(svgRef.current);
		svg.transition()
			.duration(200)
			.call(zoomBehaviorRef.current.scaleBy, 1 / 1.3);
	};

	const handleFitToView = () => {
		if (!svgRef.current || !zoomBehaviorRef.current) return;
		const svg = d3.select(svgRef.current);
		svg.transition()
			.duration(500)
			.call(
				zoomBehaviorRef.current.transform,
				d3.zoomIdentity.translate(0, 0).scale(1)
			);
	};

	const content = (
		<div className="pointer-events-auto absolute inset-0 flex flex-col overflow-visible">
			{/* Content area with padding for sidebar/inspector */}
			<div
				className="pointer-events-auto relative flex-1 overflow-visible"
				style={{
					paddingLeft: sidebarWidth,
					paddingRight: inspectorWidth,
					paddingTop: 56, // TopBar height
					transition: 'padding 0.3s ease-out'
				}}
			>
				<svg
					ref={svgRef}
					className="pointer-events-auto relative h-full w-full overflow-visible"
					style={{fontFamily: 'system-ui, sans-serif'}}
				/>

				{/* Thumb overlays positioned absolutely */}
				{thumbOverlays.map((overlay) => (
					<ThumbOverlay
						key={overlay.id}
						overlay={overlay}
						selectedFiles={selectedFiles}
						selectFileRef={selectFileRef}
						navigateToPathRef={navigateToPathRef}
						filesRef={filesRef}
						contextMenuRef={contextMenuRef}
						setContextMenuFile={setContextMenuFile}
						clickTimeoutRef={clickTimeoutRef}
						svgRef={svgRef}
						zoomBehaviorRef={zoomBehaviorRef}
						gRef={gRef}
					/>
				))}

				{/* Empty state message - only show after data has loaded */}
				{bubbleData.length === 0 &&
					!directoryQuery.isLoading &&
					dataSource && (
						<div className="pointer-events-none absolute inset-0 flex items-center justify-center">
							<p className="text-ink-dull">
								No files with size data to display
							</p>
						</div>
					)}

				{/* Floating footer controls */}
				<div
					className="bg-app-box/95 border-app-line absolute bottom-4 flex items-center gap-2 rounded-lg border p-1.5 shadow-lg backdrop-blur-lg transition-all duration-300"
					style={{ right: `${inspectorWidth + 16}px` }}
				>
					<TopBarButtonGroup>
						<TopBarButton
							icon={Minus}
							onClick={handleZoomOut}
							title="Zoom Out"
							disabled={sizeViewTransform.k <= 0.1}
						/>
						<TopBarButton
							icon={Plus}
							onClick={handleZoomIn}
							title="Zoom In"
							disabled={sizeViewTransform.k >= 100}
						/>
					</TopBarButtonGroup>
					<TopBarButton
						icon={ArrowsOut}
						onClick={handleFitToView}
						title="Fit to View"
					/>
					<TopBarButton
						icon={ArrowCounterClockwise}
						onClick={handleResetZoom}
						title="Reset Zoom"
					/>
					<div className="text-ink-dull px-2 text-xs font-medium">
						{sizeViewTransform.k.toFixed(1)}x
					</div>
				</div>
			</div>
		</div>
	);

	return portalTarget ? createPortal(content, portalTarget) : null;
}

export {SIZE_VIEW_LAYER_ID};
