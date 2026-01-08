import { Copy, Eye, Share, Trash } from "@phosphor-icons/react";
import { useContextMenu } from "@sd/interface";
import { useRef, useState } from "react";
import { useDragOperation } from "../hooks/useDragOperation";
import { useDropZone } from "../hooks/useDropZone";
import type { DragItem } from "../lib/drag";

export function DragDemo() {
  const [selectedFiles, setSelectedFiles] = useState<string[]>([
    "/Users/example/Documents/report.pdf",
    "/Users/example/Pictures/photo.jpg",
  ]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [draggingFile, setDraggingFile] = useState<string | null>(null);
  const dragStartPos = useRef<{ x: number; y: number } | null>(null);

  // Context menu for files
  const contextMenu = useContextMenu({
    items: [
      {
        icon: Copy,
        label: "Copy",
        onClick: () => alert(`Copying: ${selectedFile}`),
        keybind: "⌘C",
        condition: () => selectedFile !== null,
      },
      {
        icon: Eye,
        label: "Quick Look",
        onClick: () => alert(`Quick Look: ${selectedFile}`),
        keybind: "Space",
      },
      { type: "separator" },
      {
        icon: Share,
        label: "Share",
        submenu: [
          {
            label: "AirDrop",
            onClick: () => alert("AirDrop share"),
          },
          {
            label: "Messages",
            onClick: () => alert("Messages share"),
          },
        ],
      },
      { type: "separator" },
      {
        icon: Trash,
        label: "Delete",
        onClick: () => {
          if (selectedFile && confirm(`Delete ${selectedFile}?`)) {
            setSelectedFiles((files) =>
              files.filter((f) => f !== selectedFile)
            );
            setSelectedFile(null);
          }
        },
        keybind: "⌘⌫",
        variant: "danger" as const,
      },
    ],
  });

  const { isDragging, startDrag, cursorPosition } = useDragOperation({
    onDragStart: (sessionId) => {
      console.log("Drag started:", sessionId);
    },
    onDragEnd: (result) => {
      console.log("Drag ended:", result);
      setDraggingFile(null);
      dragStartPos.current = null;
    },
  });

  const { isHovered, dropZoneProps } = useDropZone({
    onDrop: (items) => {
      console.log("Files dropped:", items);
    },
    onDragEnter: () => {
      console.log("Drag entered drop zone");
    },
    onDragLeave: () => {
      console.log("Drag left drop zone");
    },
  });

  const handleMouseDown = (file: string, e: React.MouseEvent) => {
    setDraggingFile(file);
    dragStartPos.current = { x: e.clientX, y: e.clientY };
  };

  const handleMouseMove = async (e: React.MouseEvent) => {
    if (!(draggingFile && dragStartPos.current) || isDragging) return;

    const distance = Math.sqrt(
      (e.clientX - dragStartPos.current.x) ** 2 +
        (e.clientY - dragStartPos.current.y) ** 2
    );

    // Start native drag after moving 10px
    if (distance > 10) {
      const items: DragItem[] = [
        {
          id: `file-${draggingFile}`,
          kind: {
            type: "file" as const,
            path: draggingFile,
          },
        },
      ];

      try {
        await startDrag({
          items,
          allowedOperations: ["copy", "move"],
        });
      } catch (error) {
        console.error("Failed to start drag:", error);
        setDraggingFile(null);
      }
    }
  };

  const handleMouseUp = () => {
    setDraggingFile(null);
    dragStartPos.current = null;
  };

  return (
    <div
      className="min-h-screen space-y-6 bg-gray-900 p-8 text-white"
      onMouseLeave={handleMouseUp}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      <h1 className="font-bold text-3xl">Native Drag & Drop Demo</h1>

      {/* Draggable items */}
      <div className="space-y-4">
        <h2 className="font-semibold text-xl">Draggable Files</h2>
        <div className="space-y-2">
          {selectedFiles.map((file, idx) => (
            <div
              className={`cursor-move select-none rounded-lg border bg-gray-800 p-4 transition-colors ${
                draggingFile === file
                  ? "border-accent bg-blue-900/20"
                  : selectedFile === file
                    ? "border-green-500 bg-green-900/20"
                    : "border-gray-700 hover:border-accent"
              }`}
              key={idx}
              onClick={() => setSelectedFile(file)}
              onContextMenu={(e) => {
                setSelectedFile(file);
                contextMenu.show(e);
              }}
              onMouseDown={(e) => {
                e.preventDefault();
                handleMouseDown(file, e);
              }}
            >
              <div className="flex items-center gap-3">
                <div className="text-2xl" />
                <div className="flex-1">
                  <div className="font-medium">{file.split("/").pop()}</div>
                  <div className="text-gray-400 text-sm">{file}</div>
                </div>
              </div>
            </div>
          ))}
        </div>
        <p className="text-gray-400 text-sm">
          Click and drag these files - move them out of the window to start
          native drag!
          <br />
          Right-click on a file to test the native context menu.
        </p>
      </div>

      {/* Drop zone */}
      <div className="space-y-4">
        <h2 className="font-semibold text-xl">Drop Zone</h2>
        <div
          {...dropZoneProps}
          className={`rounded-lg border-2 border-dashed p-8 text-center transition-all ${isHovered ? "border-accent bg-accent/10" : "border-gray-700 bg-gray-800/50"}
          `}
        >
          <div className="mb-2 text-4xl">{isHovered ? "" : ""}</div>
          <div className="font-medium text-lg">
            {isHovered ? "Drop files here" : "Drag files here"}
          </div>
          <div className="mt-1 text-gray-400 text-sm">
            This drop zone accepts files from other Spacedrive windows
          </div>
        </div>
      </div>

      {/* Status */}
      <div className="space-y-2">
        <h2 className="font-semibold text-xl">Status</h2>
        <div className="space-y-2 rounded-lg bg-gray-800 p-4 font-mono text-sm">
          <div>
            <span className="text-gray-400">Dragging:</span>{" "}
            <span className={isDragging ? "text-green-400" : "text-gray-500"}>
              {isDragging ? "Yes" : "No"}
            </span>
          </div>
          <div>
            <span className="text-gray-400">Drop zone hovered:</span>{" "}
            <span className={isHovered ? "text-blue-400" : "text-gray-500"}>
              {isHovered ? "Yes" : "No"}
            </span>
          </div>
          {cursorPosition && (
            <div>
              <span className="text-gray-400">Cursor:</span>{" "}
              <span className="text-gray-300">
                ({Math.round(cursorPosition.x)}, {Math.round(cursorPosition.y)})
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="border-gray-800 border-t pt-4 text-gray-500 text-sm">
        <p className="mb-2 font-semibold">How it works:</p>
        <ul className="list-inside list-disc space-y-1">
          <li>
            Drag files from the list above to Finder - they'll appear as real
            files
          </li>
          <li>The custom overlay window follows your cursor during the drag</li>
          <li>
            Drop zones in other Spacedrive windows can receive the dragged files
          </li>
          <li>
            All drag state is synchronized across windows via Tauri events
          </li>
          <li>
            <strong>Right-click files for native context menu</strong> -
            transparent window positioned at cursor
          </li>
        </ul>
      </div>
    </div>
  );
}
