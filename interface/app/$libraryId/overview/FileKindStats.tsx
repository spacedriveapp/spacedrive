import React, { useEffect, useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import ForceGraph2D from 'react-force-graph-2d';
import { useLibraryQuery } from '@sd/client';
import { useIsDark } from '~/hooks';
import { getIcon } from '@sd/assets/util';
import * as icons from '../../../../packages/assets/icons';

interface KindStatistic {
  kind: number;
  name: string;
  count: number;
  total_bytes: string;
}

const FileKindStatistics: React.FC = () => {
  const isDark = useIsDark();
  const navigate = useNavigate();
  const { data } = useLibraryQuery(['library.kindStatistics']);
  const [graphData, setGraphData] = useState({ nodes: [], links: [] });
  const iconsRef = useRef<{ [key: string]: HTMLImageElement }>({});
  const containerRef = useRef<HTMLDivElement>(null);
  const fgRef = useRef<any>(null);

  useEffect(() => {
    if (data) {
      const statistics: KindStatistic[] = data.statistics
        .filter((item: KindStatistic) => item.kind !== 0 && item.count !== 0)
        .sort((a: KindStatistic, b: KindStatistic) => b.count - a.count);

      const totalFilesCount = statistics.reduce((sum, item) => sum + item.count, 0);
      const nodes = [
        { id: 'center', name: 'Total Files', val: totalFilesCount },
        ...statistics.map(item => ({
          id: item.kind,
          name: item.name,
          val: item.count,
        }))
      ];

      const links = statistics.map(item => ({
        source: 'center',
        target: item.kind,
      }));

      setGraphData({ nodes, links });

      // Preload icons
      statistics.forEach(item => {
        const iconName = item.name as keyof typeof icons;
        if (!iconsRef.current[iconName]) {
          const img = new Image();
          img.src = getIcon(iconName, isDark);
          iconsRef.current[iconName] = img;
        }
      });

      // d3 stuff for changing physics of the nodes
      fgRef.current.d3Force('link').distance(80); // Adjust link distance to make links shorter
      fgRef.current.d3Force('charge').strength(-1200); // Adjust charge strength
    }
  }, [data, isDark]);

  const paintNode = (node: any, ctx: CanvasRenderingContext2D, globalScale: number) => {
    const fontSize = 0.6 / globalScale;
    ctx.font = `400 ${fontSize}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    const darkColor = 'rgb(34, 34, 45)';
    const lightColor = 'rgb(252, 252, 254)';

    if (node.name === 'Total Files') {
        const radius = 25;
        const borderWidth = 0.5;

        // Create linear gradient for light mode
        const lightGradient = ctx.createLinearGradient(node.x - radius, node.y - radius, node.x + radius, node.y + radius);
        lightGradient.addColorStop(0, 'rgb(117, 177, 249)');
        lightGradient.addColorStop(1, 'rgb(0, 76, 153)');

        // Create linear gradient for dark mode
        const darkGradient = ctx.createLinearGradient(node.x - radius, node.y - radius, node.x + radius, node.y + radius);
        darkGradient.addColorStop(0, 'rgb(204, 67, 181)');
        darkGradient.addColorStop(1, 'rgb(123, 72, 188)');

        // Draw filled circle with gradient border
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
        ctx.fillStyle = isDark ? darkGradient : lightGradient;
        ctx.fill();

        // Draw inner circle to create the border effect
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius - borderWidth, 0, 2 * Math.PI, false);
        ctx.fillStyle = isDark ? darkColor : lightColor;
        ctx.fill();

        // Draw text
        ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 1)' : 'rgba(10, 10, 10, 0.8)';
        ctx.font = `bold ${fontSize * 2}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
        ctx.fillText(node.val, node.x, node.y - fontSize * 9); // Adjusted the y position to shift down

        ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(10, 10, 10, 0.8)';
        ctx.font = `400 ${fontSize * 1.1}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
        ctx.fillText(node.name, node.x, node.y + fontSize * 25);
    } else {
        const iconName = node.name as keyof typeof icons;
        const iconImg = iconsRef.current[iconName];
        const iconSize = 25 / globalScale;
        const textYPos = node.y + iconSize; // Position text below the icon

        // Draw shadow
        ctx.shadowColor = isDark ? 'rgba(230,230,230, 0.1)' : 'rgba(0, 0, 0, 0.1)';
        ctx.shadowBlur = 0.5;
        ctx.shadowOffsetX = -0.5;
        ctx.shadowOffsetY = -2;

        // Draw node circle
        const radius = 20;
        ctx.beginPath();
        ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
        ctx.fillStyle = isDark ? darkColor : lightColor;
        ctx.fill();
        ctx.shadowColor = 'transparent'; // Disable shadow for the next drawing steps

        if (iconImg) {
            ctx.drawImage(iconImg, node.x - iconSize / 2, node.y - iconSize, iconSize, iconSize);
        }

        ctx.fillStyle = isDark ? 'white' : 'black';
        ctx.fillText(node.name, node.x, textYPos - 8); // File type name in the middle

        ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(0, 0, 0, 0.5)';
        ctx.fillText(node.val, node.x, textYPos - fontSize * 2); // Number of files at the bottom
    }
};

  const handleNodeClick = (node: any) => {
    if (node.id !== 'center') {
      const path = {
        pathname: '../search',
        search: new URLSearchParams({
          filters: JSON.stringify([{ object: { kind: { in: [node.id] } } }])
        }).toString()
      };
      console.log('Navigating to:', path);
      navigate(path);
    }
  };

  const constrainNodePosition = (node: any) => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const margin = 30;

    node.x = Math.max(margin, Math.min(rect.width - margin, node.x));
    node.y = Math.max(margin, Math.min(rect.height - margin, node.y));
  };

  const paintPointerArea = (node: any, color: string, ctx: CanvasRenderingContext2D, globalScale: number) => {
    const size = 30 / globalScale; // Adjust this size to match the node size
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(node.x, node.y, size, 0, 2 * Math.PI, false);
    ctx.fill();
  };

  return (
    <div className="relative bottom-24 right-56 h-[200px] w-full" ref={containerRef}>
      {data ? (
        <ForceGraph2D
          ref={fgRef}
          graphData={graphData}
          nodeId="id"
          linkSource="source"
          linkTarget="target"
          width={1200}
          height={400}
          backgroundColor="transparent"
          nodeCanvasObject={paintNode}
          linkWidth={0.5}
          dagMode="td"
          nodeLabel=""
          linkColor={() => isDark ? 'rgba(255, 255, 255, 0.2)' : 'rgba(0, 0, 0, 0.2)'}
          onNodeClick={handleNodeClick}
          onNodeDrag={(node) => constrainNodePosition(node)}
          enableZoomInteraction={false}
          enablePanInteraction={false}
          dagLevelDistance={80}
          maxZoom={1.8}
          warmupTicks={200}
          nodePointerAreaPaint={paintPointerArea}
        />
      ) : (
        <div>Loading...</div>
      )}
    </div>
  );
};

export default FileKindStatistics;
