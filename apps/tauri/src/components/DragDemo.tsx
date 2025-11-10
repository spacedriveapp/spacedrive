import { useState, useRef } from 'react';
import { useDragOperation } from '../hooks/useDragOperation';
import { useDropZone } from '../hooks/useDropZone';
import type { DragItem } from '../lib/drag';

export function DragDemo() {
  const [selectedFiles, setSelectedFiles] = useState<string[]>([
    '/Users/example/Documents/report.pdf',
    '/Users/example/Pictures/photo.jpg',
  ]);
  const [draggingFile, setDraggingFile] = useState<string | null>(null);
  const dragStartPos = useRef<{ x: number; y: number } | null>(null);

  const { isDragging, startDrag, cursorPosition } = useDragOperation({
    onDragStart: (sessionId) => {
      console.log('Drag started:', sessionId);
    },
    onDragEnd: (result) => {
      console.log('Drag ended:', result);
      setDraggingFile(null);
      dragStartPos.current = null;
    },
  });

  const { isHovered, dropZoneProps } = useDropZone({
    onDrop: (items) => {
      console.log('Files dropped:', items);
    },
    onDragEnter: () => {
      console.log('Drag entered drop zone');
    },
    onDragLeave: () => {
      console.log('Drag left drop zone');
    },
  });

  const handleMouseDown = (file: string, e: React.MouseEvent) => {
    setDraggingFile(file);
    dragStartPos.current = { x: e.clientX, y: e.clientY };
  };

  const handleMouseMove = async (e: React.MouseEvent) => {
    if (!draggingFile || !dragStartPos.current || isDragging) return;

    const distance = Math.sqrt(
      Math.pow(e.clientX - dragStartPos.current.x, 2) +
      Math.pow(e.clientY - dragStartPos.current.y, 2)
    );

    // Start native drag after moving 10px
    if (distance > 10) {
      const items: DragItem[] = [{
        id: `file-${draggingFile}`,
        kind: {
          type: 'file' as const,
          path: draggingFile,
        },
      }];

      try {
        await startDrag({
          items,
          allowedOperations: ['copy', 'move'],
        });
      } catch (error) {
        console.error('Failed to start drag:', error);
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
      className="p-8 space-y-6 bg-gray-900 text-white min-h-screen"
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
    >
      <h1 className="text-3xl font-bold">Native Drag & Drop Demo</h1>

      {/* Draggable items */}
      <div className="space-y-4">
        <h2 className="text-xl font-semibold">Draggable Files</h2>
        <div className="space-y-2">
          {selectedFiles.map((file, idx) => (
            <div
              key={idx}
              className={`bg-gray-800 p-4 rounded-lg border transition-colors cursor-move select-none ${
                draggingFile === file
                  ? 'border-blue-500 bg-blue-900/20'
                  : 'border-gray-700 hover:border-blue-500'
              }`}
              onMouseDown={(e) => {
                e.preventDefault();
                handleMouseDown(file, e);
              }}
            >
              <div className="flex items-center gap-3">
                <div className="text-2xl"></div>
                <div className="flex-1">
                  <div className="font-medium">{file.split('/').pop()}</div>
                  <div className="text-sm text-gray-400">{file}</div>
                </div>
              </div>
            </div>
          ))}
        </div>
        <p className="text-sm text-gray-400">
          Click and drag these files - move them out of the window to start native drag!
        </p>
      </div>

      {/* Drop zone */}
      <div className="space-y-4">
        <h2 className="text-xl font-semibold">Drop Zone</h2>
        <div
          {...dropZoneProps}
          className={`
            border-2 border-dashed rounded-lg p-8 text-center transition-all
            ${isHovered ? 'border-blue-500 bg-blue-500/10' : 'border-gray-700 bg-gray-800/50'}
          `}
        >
          <div className="text-4xl mb-2">{isHovered ? '' : ''}</div>
          <div className="text-lg font-medium">
            {isHovered ? 'Drop files here' : 'Drag files here'}
          </div>
          <div className="text-sm text-gray-400 mt-1">
            This drop zone accepts files from other Spacedrive windows
          </div>
        </div>
      </div>

      {/* Status */}
      <div className="space-y-2">
        <h2 className="text-xl font-semibold">Status</h2>
        <div className="bg-gray-800 p-4 rounded-lg space-y-2 font-mono text-sm">
          <div>
            <span className="text-gray-400">Dragging:</span>{' '}
            <span className={isDragging ? 'text-green-400' : 'text-gray-500'}>
              {isDragging ? 'Yes' : 'No'}
            </span>
          </div>
          <div>
            <span className="text-gray-400">Drop zone hovered:</span>{' '}
            <span className={isHovered ? 'text-blue-400' : 'text-gray-500'}>
              {isHovered ? 'Yes' : 'No'}
            </span>
          </div>
          {cursorPosition && (
            <div>
              <span className="text-gray-400">Cursor:</span>{' '}
              <span className="text-gray-300">
                ({Math.round(cursorPosition.x)}, {Math.round(cursorPosition.y)})
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="text-sm text-gray-500 border-t border-gray-800 pt-4">
        <p className="font-semibold mb-2">How it works:</p>
        <ul className="list-disc list-inside space-y-1">
          <li>Drag files from the list above to Finder - they'll appear as real files</li>
          <li>The custom overlay window follows your cursor during the drag</li>
          <li>Drop zones in other Spacedrive windows can receive the dragged files</li>
          <li>All drag state is synchronized across windows via Tauri events</li>
        </ul>
      </div>
    </div>
  );
}
