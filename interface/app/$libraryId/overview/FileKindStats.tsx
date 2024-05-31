import React, { useEffect, useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import ForceGraph2D from 'react-force-graph-2d';
import { useLibraryQuery } from '@sd/client';
import { useIsDark } from '~/hooks';
import { getIcon } from '@sd/assets/util';
import * as icons from '../../../../packages/assets/icons'; // Adjust the import path for your icons

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

    if (node.name === 'Total Files') {
      const radius = 25;
      ctx.beginPath();
      ctx.arc(node.x, node.y, radius, 0, 2 * Math.PI, false);
      ctx.fillStyle = isDark ? 'rgb(28, 29, 37)' : 'white';
      ctx.fill();
      ctx.strokeStyle = isDark ? 'rgb(28, 29, 37)' : 'white';
      ctx.stroke();

      ctx.fillStyle = isDark ? 'white' : 'black';
      ctx.font = `bold ${fontSize * 2}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
      ctx.fillText(node.val, node.x, node.y - fontSize * 11);

      ctx.font = `500 ${fontSize}em ui-sans-serif, system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"`;
      ctx.fillText(node.name, node.x, node.y + fontSize * 13);
    } else {
      const label = node.name;
      const iconName = node.name as keyof typeof icons;
      const iconImg = iconsRef.current[iconName];
      const iconSize = 30 / globalScale;

      if (iconImg) {
        ctx.drawImage(iconImg, node.x - iconSize / 2, node.y - iconSize / 2, iconSize, iconSize);
      }

      ctx.fillStyle = isDark ? 'rgba(255, 255, 255, 0.8)' : 'rgba(0, 0, 0, 0.8)';
      ctx.fillText(node.val, node.x, node.y + iconSize / 1.3); // Number above the text
      ctx.fillText(label, node.x, node.y - iconSize / 1.3 + fontSize); // File type text
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
    const margin = 20;

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
    <div className="relative w-full h-[200px] bottom-16 right-64" ref={containerRef}>
      {data ? (
        <ForceGraph2D
          ref={fgRef}
          graphData={graphData}
          nodeId="id"
          linkSource="source"
          linkTarget="target"
          width={1200}
          height={300}
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
          dagLevelDistance={70}
          maxZoom={1.5}
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
