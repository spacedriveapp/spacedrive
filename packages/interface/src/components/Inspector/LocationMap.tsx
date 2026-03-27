import { useEffect, useRef, useState } from 'react';
import { MapPin, X } from '@phosphor-icons/react';
import type maplibregl from 'maplibre-gl';

interface LocationMapProps {
	latitude: number;
	longitude: number;
	className?: string;
}

// Convert HSL string to hex color
function hslToHex(hslString: string): string {
	const [h, s, l] = hslString
		.split(',')
		.map((v) => parseFloat(v.replace(/[^\d.]/g, '')));

	const lightness = l / 100;
	const saturation = s / 100;
	const a = saturation * Math.min(lightness, 1 - lightness);

	const f = (n: number) => {
		const k = (n + h / 30) % 12;
		const color = lightness - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
		return Math.round(255 * color)
			.toString(16)
			.padStart(2, '0');
	};

	return `#${f(0)}${f(8)}${f(4)}`;
}

// Custom vector map style matching Spacedrive's color system
function getMapStyle() {
	const computedStyle = getComputedStyle(document.documentElement);
	const getColor = (varName: string) => {
		const hsl = computedStyle.getPropertyValue(varName).trim();
		return hslToHex(`hsl(${hsl})`);
	};

	// Map Spacedrive colors to map elements
	const colors = {
		background: getColor('--color-app'), // Base map background
		water: getColor('--color-app-dark-box'), // Water bodies
		waterway: getColor('--color-app-line'), // Rivers, streams
		land: getColor('--color-app'), // Default land color
		park: getColor('--color-app-box'), // Parks and green spaces
		building: getColor('--color-app-box'), // Buildings
		buildingOutline: getColor('--color-app-line'), // Building outlines
		road: getColor('--color-app-line'), // Minor roads
		roadMajor: getColor('--color-app-selected'), // Major roads
		roadHighway: getColor('--color-app-hover'), // Highways
		roadOutline: getColor('--color-app-darker-box'), // Road outlines
		border: getColor('--color-app-line'), // Country/state borders
		text: getColor('--color-ink-dull'), // Labels
		textStroke: getColor('--color-app') // Text outline for readability
	};

	return {
		version: 8,
		sources: {
			protomaps: {
				type: 'vector',
				tiles: [
					'https://api.protomaps.com/tiles/v3/{z}/{x}/{y}.mvt?key=41392fb7515533a5'
				],
				maxzoom: 15
			}
		},
		glyphs: 'https://protomaps.github.io/basemaps-assets/fonts/{fontstack}/{range}.pbf',
		layers: [
			// Background
			{
				id: 'background',
				type: 'background',
				paint: {
					'background-color': colors.background
				}
			},
			// Water polygons (oceans, lakes)
			{
				id: 'water',
				type: 'fill',
				source: 'protomaps',
				'source-layer': 'water',
				paint: {
					'fill-color': colors.water
				}
			},
			// Natural features (parks, forests)
			{
				id: 'natural',
				type: 'fill',
				source: 'protomaps',
				'source-layer': 'natural',
				filter: ['in', 'pmap:kind', 'park', 'nature_reserve', 'wood', 'forest'],
				paint: {
					'fill-color': colors.park,
					'fill-opacity': 0.3
				}
			},
			// Landuse (parks, residential areas)
			{
				id: 'landuse',
				type: 'fill',
				source: 'protomaps',
				'source-layer': 'landuse',
				filter: ['in', 'pmap:kind', 'park', 'cemetery', 'forest', 'wood'],
				paint: {
					'fill-color': colors.park,
					'fill-opacity': 0.3
				}
			},
			// Buildings
			{
				id: 'buildings',
				type: 'fill',
				source: 'protomaps',
				'source-layer': 'buildings',
				paint: {
					'fill-color': colors.building,
					'fill-opacity': 0.7
				}
			},
			// Building outlines
			{
				id: 'buildings-outline',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'buildings',
				paint: {
					'line-color': colors.buildingOutline,
					'line-width': 0.5
				}
			},
			// Road casings (outlines)
			{
				id: 'roads-minor-casing',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'minor_road', 'other'],
				paint: {
					'line-color': colors.roadOutline,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 2, 16, 6]
				}
			},
			{
				id: 'roads-major-casing',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'major_road', 'medium_road'],
				paint: {
					'line-color': colors.roadOutline,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 3, 16, 8]
				}
			},
			{
				id: 'roads-highway-casing',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'highway', 'motorway'],
				paint: {
					'line-color': colors.roadOutline,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 4, 16, 12]
				}
			},
			// Roads
			{
				id: 'roads-minor',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'minor_road', 'other'],
				paint: {
					'line-color': colors.road,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 1, 16, 4]
				}
			},
			{
				id: 'roads-major',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'major_road', 'medium_road'],
				paint: {
					'line-color': colors.roadMajor,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 2, 16, 6]
				}
			},
			{
				id: 'roads-highway',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['in', 'pmap:kind', 'highway', 'motorway'],
				paint: {
					'line-color': colors.roadHighway,
					'line-width': ['interpolate', ['linear'], ['zoom'], 10, 3, 16, 10]
				}
			},
			// Borders
			{
				id: 'boundaries',
				type: 'line',
				source: 'protomaps',
				'source-layer': 'boundaries',
				paint: {
					'line-color': colors.border,
					'line-width': 1,
					'line-dasharray': [3, 2]
				}
			},
			// Labels - Places
			{
				id: 'place-labels',
				type: 'symbol',
				source: 'protomaps',
				'source-layer': 'places',
				layout: {
					'text-field': ['get', 'name'],
					'text-font': ['Noto Sans Regular'],
					'text-size': [
						'interpolate',
						['linear'],
						['zoom'],
						2,
						['case', ['==', ['get', 'pmap:kind'], 'country'], 10, 8],
						10,
						['case', ['==', ['get', 'pmap:kind'], 'country'], 18, 14]
					],
					'text-transform': 'uppercase',
					'text-letter-spacing': 0.1
				},
				paint: {
					'text-color': colors.text,
					'text-halo-color': colors.textStroke,
					'text-halo-width': 2
				}
			},
			// Labels - Roads
			{
				id: 'road-labels',
				type: 'symbol',
				source: 'protomaps',
				'source-layer': 'roads',
				filter: ['has', 'name'],
				layout: {
					'text-field': ['get', 'name'],
					'text-font': ['Noto Sans Regular'],
					'text-size': 10,
					'symbol-placement': 'line',
					'text-rotation-alignment': 'map'
				},
				paint: {
					'text-color': colors.text,
					'text-halo-color': colors.textStroke,
					'text-halo-width': 1.5
				}
			}
		]
	};
}

export function LocationMap({ latitude, longitude, className }: LocationMapProps) {
	const mapContainerRef = useRef<HTMLDivElement>(null);
	const mapRef = useRef<maplibregl.Map | null>(null);
	const markerRef = useRef<maplibregl.Marker | null>(null);
	const [isExpanded, setIsExpanded] = useState(false);

	useEffect(() => {
		if (!mapContainerRef.current) return;

		let map: maplibregl.Map;
		let marker: maplibregl.Marker;

		(async () => {
			const maplibregl = await import('maplibre-gl');
			await import('maplibre-gl/dist/maplibre-gl.css');

			// Create custom marker element
			const markerEl = document.createElement('div');
			markerEl.className = 'custom-marker';
			markerEl.style.cssText = `
				width: 32px;
				height: 32px;
				background: var(--color-accent);
				border: 3px solid var(--color-app);
				border-radius: 50% 50% 50% 0;
				transform: rotate(-45deg);
				box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
			`;

			const innerDot = document.createElement('div');
			innerDot.style.cssText = `
				position: absolute;
				top: 50%;
				left: 50%;
				transform: translate(-50%, -50%);
				width: 10px;
				height: 10px;
				background: var(--color-app);
				border-radius: 50%;
			`;
			markerEl.appendChild(innerDot);

			map = new maplibregl.Map({
				container: mapContainerRef.current!,
				style: getMapStyle() as any,
				center: [longitude, latitude],
				zoom: 13,
				attributionControl: false
			});

			marker = new maplibregl.Marker({ element: markerEl, anchor: 'bottom' })
				.setLngLat([longitude, latitude])
				.addTo(map);

			mapRef.current = map;
			markerRef.current = marker;
		})();

		return () => {
			if (markerRef.current) {
				markerRef.current.remove();
			}
			if (mapRef.current) {
				mapRef.current.remove();
			}
		};
	}, [latitude, longitude]);

	// Update zoom controls when expanded state changes
	useEffect(() => {
		if (mapRef.current && isExpanded) {
			mapRef.current.addControl(
				new (window as any).maplibregl.NavigationControl({
					visualizePitch: false
				}),
				'top-right'
			);
		}
	}, [isExpanded]);

	if (isExpanded) {
		return (
			<div className="fixed inset-0 z-50 flex items-center justify-center bg-app/95 backdrop-blur-sm">
				<div className="relative h-[80vh] w-[80vw] overflow-hidden rounded-lg border border-app-line bg-app">
					<button
						onClick={() => setIsExpanded(false)}
						className="absolute right-4 top-4 z-[1000] rounded-md bg-app-box/90 p-2 text-ink-dull backdrop-blur-sm transition-colors hover:bg-app-hover hover:text-ink"
					>
						<X size={20} weight="bold" />
					</button>
					<div ref={mapContainerRef} className="h-full w-full" />
					<div className="absolute bottom-4 left-4 z-[1000] rounded-md bg-app-box/90 px-3 py-2 text-xs text-ink-dull backdrop-blur-sm">
						{latitude.toFixed(6)}, {longitude.toFixed(6)}
					</div>
				</div>
			</div>
		);
	}

	return (
		<div className={className}>
			<div
				className="group relative h-32 cursor-pointer overflow-hidden rounded-t-lg"
				onClick={() => setIsExpanded(true)}
			>
				<div ref={mapContainerRef} className="h-full w-full" />
				<div className="absolute inset-0 flex items-center justify-center bg-app/20 opacity-0 transition-opacity group-hover:opacity-100">
					<div className="rounded-md bg-app-box/90 px-3 py-2 text-xs font-medium text-ink backdrop-blur-sm">
						Click to expand
					</div>
				</div>
			</div>
			<div className="flex items-center justify-between border-t border-app-line/30 px-3 py-2">
				<button
					onClick={() => setIsExpanded(true)}
					className="flex items-center gap-1.5 text-xs font-medium text-accent transition-opacity hover:opacity-80"
				>
					<MapPin size={14} weight="fill" />
					<span>
						{latitude.toFixed(4)}, {longitude.toFixed(4)}
					</span>
				</button>
				<button
					type="button"
					onClick={(e) => {
						e.stopPropagation();
						window.open(
							`https://www.google.com/maps/search/?api=1&query=${latitude},${longitude}`,
							'_blank'
						);
					}}
					className="text-xs font-medium text-accent opacity-60 transition-opacity hover:opacity-100"
				>
					Open in Maps
				</button>
			</div>
		</div>
	);
}
