import {
  ArrowCounterClockwise,
  ArrowsOut,
  Minus,
  Plus,
} from "@phosphor-icons/react";
import type { DirectorySortBy, File } from "@sd/ts-client";
import { TopBarButton, TopBarButtonGroup } from "@sd/ui";
import * as d3 from "d3";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNormalizedQuery } from "../../../../contexts/SpacedriveContext";
import { useExplorer } from "../../context";
import { useFileContextMenu } from "../../hooks/useFileContextMenu";
import { useSelection } from "../../SelectionContext";
import { formatBytes } from "../../utils";

// Cache for computed colors
const colorCache = new Map<string, string>();

// Gradient ID for folder bubbles
const FOLDER_GRADIENT_ID = "folder-accent-gradient";

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

// Get accent colors for gradient from CSS custom properties
function getAccentColors(): { faint: string; base: string; deep: string } {
  const root = document.documentElement;
  const style = getComputedStyle(root);

  // CSS variables store HSL values like "208, 100%, 57%"
  const accentFaint = style.getPropertyValue("--color-accent-faint").trim();
  const accent = style.getPropertyValue("--color-accent").trim();
  const accentDeep = style.getPropertyValue("--color-accent-deep").trim();

  return {
    faint: accentFaint ? `hsl(${accentFaint})` : "hsl(208, 100%, 64%)",
    base: accent ? `hsl(${accent})` : "hsl(208, 100%, 57%)",
    deep: accentDeep ? `hsl(${accentDeep})` : "hsl(208, 100%, 47%)",
  };
}

function getFileColorClass(file: File): string {
  if (file.kind === "Directory") return "bg-accent"; // Used for selection stroke

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
  if (
    ["js", "ts", "jsx", "tsx", "py", "rs", "go", "java", "cpp"].includes(ext)
  ) {
    return "bg-app-input";
  }

  // Archives - app-button
  if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
    return "bg-app-button";
  }

  return "bg-app-box";
}

function getFileColor(file: File): string {
  // Directories use the gradient
  if (file.kind === "Directory") {
    return `url(#${FOLDER_GRADIENT_ID})`;
  }
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
  const { currentPath, sortBy, navigateToPath, viewSettings } = useExplorer();
  const { selectedFiles, selectFile } = useSelection();

  const directoryQuery = useNormalizedQuery({
    wireMethod: "query:files.directory_listing",
    input: currentPath
      ? {
          path: currentPath,
          limit: null,
          include_hidden: false,
          sort_by: sortBy as DirectorySortBy,
          folders_first: viewSettings.foldersFirst,
        }
      : null!,
    resourceType: "file",
    enabled: !!currentPath,
    pathScope: currentPath ?? undefined,
  });

  const files = directoryQuery.data?.files || [];

  const svgRef = useRef<SVGSVGElement>(null);
  const zoomBehaviorRef = useRef<d3.ZoomBehavior<
    SVGSVGElement,
    unknown
  > | null>(null);
  const [currentZoom, setCurrentZoom] = useState(1);
  const clickTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const [contextMenuFile, setContextMenuFile] = useState<File | null>(null);

  // Create context menu for the current file
  const contextMenu = useFileContextMenu({
    file: contextMenuFile || files[0],
    selectedFiles,
    selected: contextMenuFile
      ? selectedFiles.some((f) => f.id === contextMenuFile.id)
      : false,
  });

  // Use refs for stable function references
  const selectFileRef = useRef(selectFile);
  const navigateToPathRef = useRef(navigateToPath);
  const filesRef = useRef(files);
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
    contextMenuRef.current = contextMenu;
  }, [selectFile, navigateToPath, files, contextMenu]);

  // Initialize zoom behavior once
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);

    // Only create g element if it doesn't exist
    let g = gRef.current;
    if (!g || g.empty()) {
      svg.selectAll("*").remove();

      // Add gradient definition for folder bubbles
      const defs = svg.append("defs");
      const accentColors = getAccentColors();

      const gradient = defs
        .append("radialGradient")
        .attr("id", FOLDER_GRADIENT_ID)
        .attr("cx", "30%")
        .attr("cy", "30%")
        .attr("r", "70%");

      // Highlight at top-left for 3D effect
      gradient
        .append("stop")
        .attr("offset", "0%")
        .attr("stop-color", accentColors.faint);

      gradient
        .append("stop")
        .attr("offset", "50%")
        .attr("stop-color", accentColors.base);

      gradient
        .append("stop")
        .attr("offset", "100%")
        .attr("stop-color", accentColors.deep);

      g = svg.append("g");
      gRef.current = g;
    }

    const updateTextOnZoom = (scale: number) => {
      // Update text transform for constant size
      g.selectAll<SVGTextElement, any>("text").attr(
        "transform",
        `scale(${1 / scale})`
      );

      // Update text content based on effective radius
      g.selectAll<SVGGElement, any>("g.bubble-node").each(function (d: any) {
        const node = d3.select(this);
        const textElement = node.select("text");
        const effectiveRadius = d.r * scale;

        textElement.selectAll("tspan").remove();

        if (effectiveRadius < 25) return;

        const nameTspan = textElement
          .append("tspan")
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
          textElement
            .append("tspan")
            .attr("x", 0)
            .attr("y", 5)
            .attr("font-size", "10px")
            .attr("fill-opacity", 0.8)
            .text(d.data.type);

          textElement
            .append("tspan")
            .attr("x", 0)
            .attr("y", 20)
            .attr("font-size", effectiveRadius > 80 ? "14px" : "12px")
            .attr("font-weight", "700")
            .text(formatBytes(d.data.value));
        }
      });
    };

    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
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
      svg
        .transition()
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

  // Reset zoom when path changes
  useEffect(() => {
    if (!(svgRef.current && zoomBehaviorRef.current)) return;
    const svg = d3.select(svgRef.current);
    svg.call(zoomBehaviorRef.current.transform, d3.zoomIdentity);
    setCurrentZoom(1);
  }, [currentPath]);

  const bubbleData = useMemo(() => {
    const filesWithSize = files.filter((f) => f.size > 0);

    if (filesWithSize.length === 0) return [];

    return filesWithSize
      .sort((a, b) => b.size - a.size)
      .slice(0, 50)
      .map((file) => ({
        id: file.id,
        name: file.name,
        value: file.size,
        file,
        color: getFileColor(file),
        type: getFileType(file),
      }));
  }, [files]);

  // Update chart data (preserves zoom state)
  useEffect(() => {
    if (!(svgRef.current && gRef.current)) return;

    const g = gRef.current;
    const width = svgRef.current.clientWidth;
    const height = svgRef.current.clientHeight;

    // Clear bubbles if no data or no dimensions
    if (bubbleData.length === 0 || width === 0 || height === 0) {
      g.selectAll("g.bubble-node").remove();
      return;
    }

    const pack = d3.pack().size([width, height]).padding(3);

    const root = pack(
      d3.hierarchy({ children: bubbleData }).sum((d) => d.value)
    );

    // Update nodes with data join (preserves existing nodes when possible)
    const nodes = g
      .selectAll<SVGGElement, any>("g.bubble-node")
      .data(root.leaves(), (d: any) => d.data.id)
      .join(
        (enter) =>
          enter
            .append("g")
            .attr("class", "bubble-node")
            .attr("transform", (d) => `translate(${d.x},${d.y})`)
            .style("cursor", "pointer"),
        (update) => update.attr("transform", (d) => `translate(${d.x},${d.y})`),
        (exit) => exit.remove()
      );

    // Update or create circles
    nodes
      .selectAll<SVGCircleElement, any>("circle")
      .data((d) => [d])
      .join("circle")
      .attr("r", (d) => d.r)
      .attr("fill", (d) => d.data.color)
      .attr("fill-opacity", 1)
      .attr("stroke", "transparent")
      .attr("stroke-width", 0)
      .attr("data-file-id", (d) => d.data.id)
      .on("click", (event, d) => {
        event.stopPropagation();

        const multi = event.metaKey || event.ctrlKey;
        const range = event.shiftKey;

        // Select immediately for responsive feedback
        selectFileRef.current(d.data.file, filesRef.current, multi, range);

        // Clear any existing zoom timeout
        if (clickTimeoutRef.current) {
          clearTimeout(clickTimeoutRef.current);
          clickTimeoutRef.current = null;
        }

        // Delay zoom-to-focus to allow double-click detection
        if (!(multi || range) && svgRef.current && zoomBehaviorRef.current) {
          clickTimeoutRef.current = setTimeout(() => {
            if (!(svgRef.current && zoomBehaviorRef.current)) return;

            const svgElement = svgRef.current;
            const width = svgElement.clientWidth;
            const height = svgElement.clientHeight;
            const centerX = width / 2;
            const centerY = height / 2;

            // Target: make the bubble appear at a consistent size on screen
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
          navigateToPathRef.current(d.data.file.sd_path);
        }
      })
      .on("contextmenu", async (event, d) => {
        event.preventDefault();
        event.stopPropagation();

        // Select the file if not already selected
        const isSelected = selectedFiles.some((f) => f.id === d.data.file.id);
        if (!isSelected) {
          selectFileRef.current(d.data.file, filesRef.current, false, false);
        }

        // Set the context menu file and show menu
        setContextMenuFile(d.data.file);

        // Show context menu on next tick after state updates
        setTimeout(async () => {
          await contextMenuRef.current.show(event);
        }, 0);
      })
      .on("mouseenter", function (event, d) {
        d3.select(this)
          .transition()
          .duration(150)
          .attr("filter", "brightness(1.15)");
      })
      .on("mouseleave", function (event, d) {
        d3.select(this).transition().duration(150).attr("filter", null);
      });

    // Update or create titles
    nodes
      .selectAll<SVGTitleElement, any>("title")
      .data((d) => [d])
      .join("title")
      .text((d) => `${d.data.name}\n${formatBytes(d.data.value)}`);

    // Update or create text elements
    nodes
      .selectAll<SVGTextElement, any>("text")
      .data((d) => [d])
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
      g.selectAll<SVGTextElement, any>("text").attr(
        "transform",
        `scale(${1 / scale})`
      );

      nodes.each(function (d) {
        const node = d3.select(this);
        const textElement = node.select("text");
        const effectiveRadius = d.r * scale;

        textElement.selectAll("tspan").remove();

        if (effectiveRadius < 25) return;

        const nameTspan = textElement
          .append("tspan")
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
          textElement
            .append("tspan")
            .attr("x", 0)
            .attr("y", 5)
            .attr("font-size", "10px")
            .attr("fill-opacity", 0.8)
            .text(d.data.type);

          textElement
            .append("tspan")
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

    svg
      .selectAll<SVGCircleElement, any>("circle[data-file-id]")
      .attr("stroke", (d) => {
        const isSelected = selectedFiles.some((f) => f.id === d.data.id);
        return isSelected ? accentColor : "transparent";
      })
      .attr("stroke-width", (d) => {
        const isSelected = selectedFiles.some((f) => f.id === d.data.id);
        return isSelected ? 4 : 0;
      });
  }, [selectedFiles]);

  const handleResetZoom = () => {
    if (!(svgRef.current && zoomBehaviorRef.current)) return;
    const svg = d3.select(svgRef.current);
    svg
      .transition()
      .duration(300)
      .call(zoomBehaviorRef.current.transform, d3.zoomIdentity)
      .on("end", () => setCurrentZoom(1));
  };

  const handleZoomIn = () => {
    if (!(svgRef.current && zoomBehaviorRef.current)) return;
    const svg = d3.select(svgRef.current);
    svg.transition().duration(200).call(zoomBehaviorRef.current.scaleBy, 1.3);
  };

  const handleZoomOut = () => {
    if (!(svgRef.current && zoomBehaviorRef.current)) return;
    const svg = d3.select(svgRef.current);
    svg
      .transition()
      .duration(200)
      .call(zoomBehaviorRef.current.scaleBy, 1 / 1.3);
  };

  const handleFitToView = () => {
    if (!(svgRef.current && zoomBehaviorRef.current)) return;
    const svg = d3.select(svgRef.current);
    svg
      .transition()
      .duration(500)
      .call(
        zoomBehaviorRef.current.transform,
        d3.zoomIdentity.translate(0, 0).scale(1)
      );
  };

  return (
    <div className="relative h-full w-full overflow-hidden">
      <svg
        className="relative h-full w-full"
        ref={svgRef}
        style={{ fontFamily: "system-ui, sans-serif" }}
      />

      {/* Empty state message */}
      {bubbleData.length === 0 && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
          <p className="text-ink-dull">No files with size data to display</p>
        </div>
      )}

      {/* Floating footer controls */}
      <div className="absolute right-4 bottom-4 flex items-center gap-2 rounded-lg border border-app-line bg-app-box/95 p-1.5 shadow-lg backdrop-blur-lg">
        <TopBarButtonGroup>
          <TopBarButton
            disabled={currentZoom <= 0.1}
            icon={Minus}
            onClick={handleZoomOut}
            title="Zoom Out"
          />
          <TopBarButton
            disabled={currentZoom >= 100}
            icon={Plus}
            onClick={handleZoomIn}
            title="Zoom In"
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
        <div className="px-2 font-medium text-ink-dull text-xs">
          {currentZoom.toFixed(1)}x
        </div>
      </div>
    </div>
  );
}
