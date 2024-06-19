import { useMemo } from 'react';
import { nonIndexedPathOrderingSchema, useDiscoveredPeers } from '@sd/client';
import { Icon } from '~/components';
import { useLocale } from '~/hooks';
import { useRouteTitle } from '~/hooks/useRouteTitle';

import Explorer from './Explorer';
import { ExplorerContextProvider } from './Explorer/Context';
import { createDefaultExplorerSettings } from './Explorer/store';
import { DefaultTopBarOptions } from './Explorer/TopBarOptions';
import { useExplorer, useExplorerSettings } from './Explorer/useExplorer';
import { TopBarPortal } from './TopBar/Portal';

export const Component = () => {
	const title = useRouteTitle('Network');

	const { t } = useLocale();

	const discoveredPeers = useDiscoveredPeers();
	const peers = useMemo(() => Array.from(discoveredPeers.values()), [discoveredPeers]);

	const explorerSettings = useExplorerSettings({
		settings: useMemo(
			() =>
				createDefaultExplorerSettings({
					order: {
						field: 'name',
						value: 'Asc'
					}
				}),
			[]
		),
		orderingKeys: nonIndexedPathOrderingSchema
	});

	const explorer = useExplorer({
		items: peers.map((peer) => ({
			type: 'SpacedropPeer' as const,
			has_local_thumbnail: false,
			thumbnail: null,
			item: {
				...peer.metadata,
				pub_id: []
			}
		})),
		settings: explorerSettings,
		layouts: { media: false }
	});

	return (
		<ExplorerContextProvider explorer={explorer}>
			<TopBarPortal
				left={
					<div className="flex items-center gap-2">
						<Icon name="Globe" size={22} />
						<span className="truncate text-sm font-medium">{title}</span>
					</div>
				}
				right={<DefaultTopBarOptions />}
			/>
			<Explorer
				emptyNotice={
					<div className="flex h-full flex-col items-center justify-center text-white">
						<Icon name="Globe" size={128} />
						<h1 className="mt-4 text-lg font-bold">{t('your_local_network')}</h1>
						<p className="mt-1 max-w-sm text-center text-sm text-ink-dull">
							{t('network_page_description')}
						</p>
					</div>
				}
			/>
		</ExplorerContextProvider>
	);
};

// NOTE -> this is code for the node graph. The plan is to implement this in network (moved from overview page). Jamie asked me to save the code somewhere
// so placing it here for now!

// import { getIcon } from '@sd/assets/util';
// import { useLibraryQuery } from '@sd/client';
// import React, { useEffect, useRef, useState, useCallback } from 'react';
// import { useIsDark } from '~/hooks';
// import ForceGraph2D from 'react-force-graph-2d';
// import { useNavigate } from 'react-router';
// import * as d3 from 'd3-force';

// //million-ignore
// const canvasWidth = 700
// const canvasHeight = 600;

// interface KindStatistic {
//   kind: number;
//   name: string;
//   count: number;
//   total_bytes: string;
// }

// interface Node {
//   id: string | number;
//   name: string;
//   val: number;
//   fx?: number;
//   fy?: number;
//   x?: number;
//   y?: number;
// }

// interface Link {
//   source: string | number;
//   target: string | number;
// }

// interface GraphData {
//   nodes: Node[];
//   links: Link[];
// }

// const FileKindStatistics: React.FC = () => {
//   const isDark = useIsDark();
//   const navigate = useNavigate();
//   const { data } = useLibraryQuery(['library.kindStatistics']);
//   const [graphData, setGraphData] = useState<GraphData>({ nodes: [], links: [] });
//   const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});
//   const containerRef = useRef<HTMLDivElement>(null);
//   const fgRef = useRef<any>(null);

//   useEffect(() => {
//     if (data) {
//       const statistics: KindStatistic[] = data.statistics
//         .filter((item: KindStatistic) => item.count != 0)
//         .sort((a: KindStatistic, b: KindStatistic) => b.count - a.count)
// 		// TODO: eventually allow users to select and save which file kinds are shown
//         .slice(0, 18); // Get the top 18 highest file kinds

//       const totalFilesCount = statistics.reduce((sum, item) => sum + item.count, 0);
//       const nodes = [
//         { id: 'center', name: 'Total Files', val: totalFilesCount },
//         ...statistics.map(item => ({
//           id: item.kind,
//           name: item.name,
//           val: item.count,
//         }))
//       ];

//       const links = statistics.map(item => ({
//         source: 'center',
//         target: item.kind,
//       }));

//       setGraphData({ nodes, links });

//       // Preload icons, this is for rendering purposes
//       statistics.forEach(item => {
//         const iconName = item.name;
//         if (!iconsRef.current[iconName]) {
//           const img = new Image();
//           img.src = getIcon(iconName, isDark);
//           iconsRef.current[iconName] = img;
//         }
//       });

//       // d3 stuff for changing physics of the nodes
//       fgRef.current.d3Force('link').distance(110); // Adjust link distance to make links shorter
//       fgRef.current.d3Force('charge').strength(-50); // how hard the nodes repel
//       fgRef.current.d3Force('center').strength(10); // Adjust center strength for stability
//       fgRef.current.d3Force('collision', d3.forceCollide().radius(25)); // Add collision force with radius. Should be a little larger than radius of nodes.

//       fgRef.current.d3Force('y', d3.forceY(canvasHeight / 5).strength((1.2))); // strong force to ensure nodes don't spill out of canvas
//     }
//   }, [data, isDark]);

//   const paintNode = useCallback((node: any, ctx: CanvasRenderingContext2D, globalScale: number) => {
//     const fontSize = 0.6 / globalScale;
//     ctx.font = `400 ${fontSize}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
//     ctx.textAlign = 'center';
//     ctx.textBaseline = 'middle';

//     const darkColor = 'rgb(34, 34, 45)';
//     const lightColor = 'rgb(252, 252, 254)';

// 	const x = isFinite(node.x) ? node.x : 0;
// 	const y = isFinite(node.y) ? node.y : 0;

//     if (node.name === 'Total Files') {
//       const radius = 25;
//       const borderWidth = 0.5;

//       // Create linear gradient for light mode
// 	  const lightGradient = ctx.createLinearGradient(x - radius, y - radius, x + radius, y + radius);
// 	  lightGradient.addColorStop(0, 'rgb(117, 177, 249)');
// 	  lightGradient.addColorStop(1, 'rgb(0, 76, 153)');

// 	  // Create linear gradient for dark mode
// 	  const darkGradient = ctx.createLinearGradient(x - radius, y - radius, x + radius, y + radius);
// 	  darkGradient.addColorStop(0, 'rgb(255, 13, 202)');
// 	  darkGradient.addColorStop(1, 'rgb(128, 0, 255)');

//       // Draw filled circle with gradient border
//       ctx.beginPath();
//       ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
//       ctx.fillStyle = isDark ? darkGradient : lightGradient;
//       ctx.fill();

//       // Draw inner circle to create the border effect
//       ctx.beginPath();
//       ctx.arc(node.x, node.y, radius - borderWidth, 0, 2 * Math.PI, false);
//       ctx.fillStyle = isDark ? darkColor : lightColor;
//       ctx.fill();

//       // Add inner shadow
//       const shadowGradient = ctx.createRadialGradient(x, y, radius * 0.5, x, y, radius);
//       shadowGradient.addColorStop(0, 'rgba(0, 0, 0, 0)');
//       shadowGradient.addColorStop(1, isDark ? 'rgba(255, 93, 234, 0.1' : 'rgba(66, 97, 255, 0.05)');

//       ctx.globalCompositeOperation = 'source-atop';
//       ctx.beginPath();
//       ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
//       ctx.fillStyle = shadowGradient;
//       ctx.fill();

//       // Draw text
//       ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 1)' : 'rgba(10, 10, 10, 0.8)';
//       ctx.font = `bold ${fontSize * 2}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
//       ctx.fillText(node.val, node.x, node.y - fontSize * 9);

//       ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(10, 10, 10, 0.8)';
//       ctx.font = `400 ${fontSize * 1.1}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
//       ctx.fillText(node.name, node.x, node.y + fontSize * 25);
//     } else {
//       const iconName = node.name;
//       const iconImg = iconsRef.current[iconName];
//       const iconSize = 25 / globalScale;
//       const textYPos = node.y + iconSize;

//       // Draw shadow
//       ctx.shadowColor = isDark ? 'rgb(44, 45, 58)' : 'rgba(0, 0, 0, 0.1)';
//       ctx.shadowBlur = 0.5;
//       ctx.shadowOffsetX = -0.5;
//       ctx.shadowOffsetY = -2;

//       // Draw node circle
//       const radius = 18;
//       ctx.beginPath();
//       ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
//       ctx.fillStyle = isDark ? darkColor : lightColor;
//       ctx.fill();
//       ctx.shadowColor = 'transparent';

//       if (iconImg) {
//         ctx.drawImage(iconImg, node.x - iconSize / 2, node.y - iconSize, iconSize, iconSize);
//       }

//       ctx.fillStyle = isDark ? 'white' : 'black';

//       // Truncate node name if it is too long
//       let truncatedName = node.name;
//       if (node.name.length > 10) {
//         truncatedName = node.name.slice(0, 6) + "...";
//       }
//       ctx.fillText(truncatedName, node.x, textYPos - 9);

//       ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.5)';
//       ctx.fillText(node.val, node.x, textYPos - 2);
//     }
//   }, [isDark]);

//   const handleNodeClick = useCallback((node: any) => {
//     if (node.id !== 'center') {
//       const path = {
//         pathname: '../search',
//         search: new URLSearchParams({
//           filters: JSON.stringify([{ object: { kind: { in: [node.id] } } }])
//         }).toString()
//       };
//       navigate(path);
//     }
//   }, [navigate]);

//   const handleEngineTick = () => {
// 	const centerNode = graphData.nodes.find((node: any) => node.id === 'center');
// 		if (centerNode) {
// 		  centerNode.fx = 0;
// 		  centerNode.fy = 0;
// 		}
//   }

//   useEffect(() => {
//     if (fgRef.current) {
//       fgRef.current.d3Force('center', d3.forceCenter());
//     }
//   }, []);

//   const paintPointerArea = useCallback((node: any, color: string, ctx: CanvasRenderingContext2D, globalScale: number) => {
//     const size = 30 / globalScale;
//     ctx.fillStyle = color;
//     ctx.beginPath();
//     ctx.arc(node.x, node.y, size, 0, 2 * Math.PI, false);
//     ctx.fill();
//   }, []);

//   return (
//     <div className="relative bottom-48 h-[200px] w-full" ref={containerRef}>
//       {data ? (
//         <ForceGraph2D
//           ref={fgRef}
//           graphData={graphData}
//           nodeId="id"
//           linkSource="source"
//           linkTarget="target"
//           width={canvasWidth}
//           height={canvasHeight}
//           backgroundColor="transparent"
//           nodeCanvasObject={paintNode}
//           linkWidth={0.5}
//           nodeLabel=""
// 		  dagMode="radialout"
//           linkColor={() => isDark ? '#2C2D3A' : 'rgba(0, 0, 0, 0.2)'}
//           onNodeClick={handleNodeClick}
//           enableZoomInteraction={false}
//           enablePanInteraction={false}
//           dagLevelDistance={100}
//           warmupTicks={500}
//           d3VelocityDecay={0.75}
// 		  onEngineTick={handleEngineTick}
//           nodePointerAreaPaint={paintPointerArea}
//         />
//       ) : (
//         <div>Loading...</div>
//       )}
//     </div>
//   );
// };

// export default React.memo(FileKindStatistics);
