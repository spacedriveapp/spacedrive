import { useEffect, useRef, useMemo, useState } from "react";
import * as d3 from "d3";
import type { File } from "@sd/ts-client/generated/types";
import { useExplorer } from "../../context";
import { formatBytes } from "../../utils";
import { TopBarButton, TopBarButtonGroup } from "@sd/ui";
import { ArrowsOut, ArrowCounterClockwise, Plus, Minus } from "@phosphor-icons/react";

// Cache for computed colors
const colorCache = new Map<string, string>();

// Get computed color from Tailwind class
function getTailwindColor(className: string): string {
  if (colorCache.has(className)) {
    return colorCache.get(className)!;
  }

  const div = document.createElement("div");
  div.className = className;
  div.style.display = "none";
  document.body.appendChild(div);
  const color = getComputedStyle(div).backgroundColor;
  document.body.removeChild(div);

  colorCache.set(className, color);
  return color;
}

function getFileColorClass(file: File): string {
  if (file.kind === "Directory") return "bg-accent";

  const ext = file.name.split(".").pop()?.toLowerCase() || "";

  // Images - lighter app-box
  if (["jpg", "jpeg", "png", "gif", "svg", "webp", "heic"].includes(ext)) {
    return "bg-app-light-box";
  }

  // Videos - app-selected
  if (["mp4", "mov", "avi", "mkv", "webm"].includes(ext)) {
    return "bg-app-selected";
  }

  // Audio - app-hover
  if (["mp3", "wav", "flac", "aac", "ogg"].includes(ext)) {
    return "bg-app-hover";
  }

  // Documents - app-active
  if (["pdf", "doc", "docx", "txt", "md"].includes(ext)) {
    return "bg-app-active";
  }

  // Code - app-input
  if (["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "cpp"].includes(ext)) {
    return "bg-app-input";
  }

  // Archives - app-button
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
    return "bg-app-button";
  }

  return "bg-app-box";
}

function getFileColor(file: File): string {
  return getTailwindColor(getFileColorClass(file));
}

function getFileType(file: File): string {
  if (file.kind === "Directory") return "Folder";

  const name = file.name;
  const lastDot = name.lastIndexOf(".");
  if (lastDot === -1 || lastDot === 0) return "File";

  return name.slice(lastDot + 1).toUpperCase();
}

export function SizeView() {
  const { files, selectedFiles, selectFile, setCurrentPath } = useExplorer();
  const svgRef = useRef<SVGSVGElement>(null);
  const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const [currentZoom, setCurrentZoom] = useState(1);
  const clickTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Use refs for stable function references
  const selectFileRef = useRef(selectFile);
  const setCurrentPathRef = useRef(setCurrentPath);
  const gRef = useRef<d3.Selection<SVGGElement, unknown, null, undefined> | null>(null);

  useEffect(() => {
    selectFileRef.current = selectFile;
    setCurrentPathRef.current = setCurrentPath;
  }, [selectFile, setCurrentPath]);

  // Initialize zoom behavior once
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);

    // Only create g element if it doesn't exist
    let g = gRef.current;
    if (!g || g.empty()) {
      svg.selectAll("*").remove();
      g = svg.append("g");
      gRef.current = g;
    }

    const updateTextOnZoom = (scale: number) => {
      // Update text transform for constant size
      g.selectAll<SVGTextElement, any>("text")
        .attr("transform", `scale(${1 / scale})`);

      // Update text content based on effective radius
      g.selectAll<SVGGElement, any>("g.bubble-node").each(function(d: any) {
        const node = d3.select(this);
        const textElement = node.select("text");
        const effectiveRadius = d.r * scale;

        textElement.selectAll("tspan").remove();

        if (effectiveRadius < 25) return;

        const nameTspan = textElement.append("tspan")
          .attr("x", 0)
          .attr("y", effectiveRadius > 40 ? -10 : 0);

        if (effectiveRadius > 80) {
          nameTspan.attr("font-size", "14px");
        } else if (effectiveRadius > 50) {
          nameTspan.attr("font-size", "12px");
        } else {
          nameTspan.attr("font-size", "10px");
        }

        const maxLength = Math.floor(effectiveRadius / 5);
        nameTspan.text(
          d.data.name.length > maxLength
            ? d.data.name.slice(0, maxLength) + "..."
            : d.data.name
        );

        if (effectiveRadius > 40) {
          textElement.append("tspan")
            .attr("x", 0)
            .attr("y", 5)
            .attr("font-size", "10px")
            .attr("fill-opacity", 0.8)
            .text(d.data.type);

          textElement.append("tspan")
            .attr("x", 0)
            .attr("y", 20)
            .attr("font-size", effectiveRadius > 80 ? "14px" : "12px")
            .attr("font-weight", "700")
            .text(formatBytes(d.data.value));
        }
      });
    };

    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 100])
      .on("zoom", (event) => {
        g.attr("transform", event.transform);
        setCurrentZoom(event.transform.k);
        updateTextOnZoom(event.transform.k);
      });

    svg.call(zoom);
    zoomBehaviorRef.current = zoom;

    // Double-click to reset zoom
    svg.on("dblclick.zoom", () => {
      svg.transition()
        .duration(300)
        .call(zoom.transform, d3.zoomIdentity)
        .on("end", () => {
          setCurrentZoom(1);
          updateTextOnZoom(1);
        });
    });

    return () => {
      svg.selectAll("*").remove();
      gRef.current = null;
      if (clickTimeoutRef.current) {
        clearTimeout(clickTimeoutRef.current);
      }
    };
  }, []); // Only run once

  const bubbleData = useMemo(() => {
    const filesWithSize = files.filter(f => f.size > 0);

    if (filesWithSize.length === 0) return [];

    return filesWithSize
      .sort((a, b) => b.size - a.size)
      .slice(0, 50)
      .map(file => ({
        id: file.id,
        name: file.name,
        value: file.size,
        file,
        color: getFileColor(file),
        type: getFileType(file)
      }));
  }, [files]);

  // Update chart data (preserves zoom state)
  useEffect(() => {
    if (!svgRef.current || !gRef.current) return;

    const g = gRef.current;
    const width = svgRef.current.clientWidth;
    const height = svgRef.current.clientHeight;

    // Clear bubbles if no data or no dimensions
    if (bubbleData.length === 0 || width === 0 || height === 0) {
      g.selectAll("g.bubble-node").remove();
      return;
    }

    const pack = d3.pack()
      .size([width, height])
      .padding(3);

    const root = pack(
      d3.hierarchy({ children: bubbleData })
        .sum(d => d.value)
    );

    // Update nodes with data join (preserves existing nodes when possible)
    const nodes = g
      .selectAll<SVGGElement, any>("g.bubble-node")
      .data(root.leaves(), (d: any) => d.data.id)
      .join(
        enter => enter.append("g")
          .attr("class", "bubble-node")
          .attr("transform", d => `translate(${d.x},${d.y})`)
          .style("cursor", "pointer"),
        update => update
          .attr("transform", d => `translate(${d.x},${d.y})`),
        exit => exit.remove()
      );

    // Find min and max radius for opacity scaling
    const radii = root.leaves().map(d => d.r);
    const minRadius = Math.min(...radii);
    const maxRadius = Math.max(...radii);

    // Create opacity scale
    const opacityScale = d3.scaleLinear()
      .domain([minRadius, maxRadius])
      .range([0.3, 0.8])
      .clamp(true);

    // Update or create circles
    nodes.selectAll<SVGCircleElement, any>("circle")
      .data(d => [d])
      .join("circle")
      .attr("r", d => d.r)
      .attr("fill", d => d.data.color)
      .attr("fill-opacity", d => opacityScale(d.r))
      .attr("stroke", "transparent")
      .attr("stroke-width", 0)
      .attr("data-file-id", d => d.data.id)
      .on("click", (event, d) => {
        event.stopPropagation();

        // Clear any existing timeout
        if (clickTimeoutRef.current) {
          clearTimeout(clickTimeoutRef.current);
          clickTimeoutRef.current = null;
        }

        // Set timeout for single click
        clickTimeoutRef.current = setTimeout(() => {
          const multi = event.metaKey || event.ctrlKey;
          const range = event.shiftKey;
          selectFileRef.current(d.data.file, multi, range);

          // Zoom to center this circle
          if (!multi && !range && svgRef.current && zoomBehaviorRef.current) {
            const svgElement = svgRef.current;
            const width = svgElement.clientWidth;
            const height = svgElement.clientHeight;

            // Calculate the transform needed to center this circle
            const currentTransform = d3.zoomTransform(svgElement);
            const centerX = width / 2;
            const centerY = height / 2;

            // Target: make the bubble appear at a consistent size on screen
            // regardless of its original size
            const targetBubbleScreenSize = Math.min(width, height) * 0.4; // 40% of viewport
            const bubbleSize = d.r * 2; // diameter in data coordinates

            // Calculate what scale would make this bubble that size on screen
            const targetScale = targetBubbleScreenSize / bubbleSize;

            // Create new transform
            const newTransform = d3.zoomIdentity
              .translate(centerX, centerY)
              .scale(targetScale)
              .translate(-d.x, -d.y);

            d3.select(svgElement)
              .transition()
              .duration(500)
              .call(zoomBehaviorRef.current.transform, newTransform);
          }
        }, 250); // 250ms delay to detect double click
      })
      .on("dblclick", (event, d) => {
        event.stopPropagation();

        // Clear single click timeout
        if (clickTimeoutRef.current) {
          clearTimeout(clickTimeoutRef.current);
          clickTimeoutRef.current = null;
        }

        // Navigate if directory
        if (d.data.file.kind === "Directory") {
          setCurrentPathRef.current(d.data.file.sd_path);
        }
      })
      .on("mouseenter", function(event, d) {
        d3.select(this)
          .transition()
          .duration(150)
          .attr("fill-opacity", Math.min(opacityScale(d.r) + 0.2, 1));
      })
      .on("mouseleave", function(event, d) {
        d3.select(this)
          .transition()
          .duration(150)
          .attr("fill-opacity", opacityScale(d.r));
      });

    // Update or create titles
    nodes.selectAll<SVGTitleElement, any>("title")
      .data(d => [d])
      .join("title")
      .text(d => `${d.data.name}\n${formatBytes(d.data.value)}`);

    // Update or create text elements
    nodes.selectAll<SVGTextElement, any>("text")
      .data(d => [d])
      .join("text")
      .attr("text-anchor", "middle")
      .attr("fill", "white")
      .attr("font-weight", "600")
      .style("pointer-events", "none");

    // Trigger text update with current zoom level
    if (svgRef.current) {
      const currentTransform = d3.zoomTransform(svgRef.current);
      const scale = currentTransform.k;

      // Update text transform and content
      g.selectAll<SVGTextElement, any>("text")
        .attr("transform", `scale(${1 / scale})`);

      nodes.each(function(d) {
        const node = d3.select(this);
        const textElement = node.select("text");
        const effectiveRadius = d.r * scale;

        textElement.selectAll("tspan").remove();

        if (effectiveRadius < 25) return;

        const nameTspan = textElement.append("tspan")
          .attr("x", 0)
          .attr("y", effectiveRadius > 40 ? -10 : 0);

        if (effectiveRadius > 80) {
          nameTspan.attr("font-size", "14px");
        } else if (effectiveRadius > 50) {
          nameTspan.attr("font-size", "12px");
        } else {
          nameTspan.attr("font-size", "10px");
        }

        const maxLength = Math.floor(effectiveRadius / 5);
        nameTspan.text(
          d.data.name.length > maxLength
            ? d.data.name.slice(0, maxLength) + "..."
            : d.data.name
        );

        if (effectiveRadius > 40) {
          textElement.append("tspan")
            .attr("x", 0)
            .attr("y", 5)
            .attr("font-size", "10px")
            .attr("fill-opacity", 0.8)
            .text(d.data.type);

          textElement.append("tspan")
            .attr("x", 0)
            .attr("y", 20)
            .attr("font-size", effectiveRadius > 80 ? "14px" : "12px")
            .attr("font-weight", "700")
            .text(formatBytes(d.data.value));
        }
      });
    }
  }, [bubbleData]);

  // Update selection strokes when selectedFiles changes
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);
    const accentColor = getTailwindColor("bg-accent");

    svg.selectAll<SVGCircleElement, any>("circle[data-file-id]")
      .attr("stroke", d => {
        const isSelected = selectedFiles.some(f => f.id === d.data.id);
        return isSelected ? accentColor : "transparent";
      })
      .attr("stroke-width", d => {
        const isSelected = selectedFiles.some(f => f.id === d.data.id);
        return isSelected ? 4 : 0;
      });
  }, [selectedFiles]);

  const handleResetZoom = () => {
    if (!svgRef.current || !zoomBehaviorRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.transition()
      .duration(300)
      .call(zoomBehaviorRef.current.transform, d3.zoomIdentity)
      .on("end", () => setCurrentZoom(1));
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

  return (
    <div className="relative w-full h-full overflow-hidden">
      <svg
        ref={svgRef}
        className="w-full h-full"
        style={{ fontFamily: "system-ui, sans-serif" }}
      />

      {/* Empty state message */}
      {bubbleData.length === 0 && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <p className="text-ink-dull">No files with size data to display</p>
        </div>
      )}

      {/* Floating footer controls */}
      <div className="absolute bottom-4 right-4 flex items-center gap-2 bg-app-box/95 backdrop-blur-lg border border-app-line rounded-lg p-1.5 shadow-lg">
        <TopBarButtonGroup>
          <TopBarButton
            icon={Minus}
            onClick={handleZoomOut}
            title="Zoom Out"
            disabled={currentZoom <= 0.1}
          />
          <TopBarButton
            icon={Plus}
            onClick={handleZoomIn}
            title="Zoom In"
            disabled={currentZoom >= 100}
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
        <div className="px-2 text-xs text-ink-dull font-medium">
          {currentZoom.toFixed(1)}x
        </div>
      </div>
    </div>
  );
}
