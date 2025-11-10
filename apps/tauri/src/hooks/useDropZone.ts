import { useEffect, useState, useCallback, useRef } from 'react';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  onDragEntered,
  onDragLeft,
  onDragEnded,
  type DragItem,
  type DragResult,
} from '../lib/drag';

export interface UseDropZoneOptions {
  onDrop?: (items: DragItem[]) => void;
  onDragEnter?: () => void;
  onDragLeave?: () => void;
}

export function useDropZone(options: UseDropZoneOptions = {}) {
  const [isHovered, setIsHovered] = useState(false);
  const [dragItems, setDragItems] = useState<DragItem[]>([]);
  const currentWindowLabel = getCurrentWebviewWindow().label;
  const currentSessionRef = useRef<string | null>(null);

  useEffect(() => {
    const unlistenEntered = onDragEntered((event) => {
      if (event.windowLabel === currentWindowLabel) {
        setIsHovered(true);
        options.onDragEnter?.();
      }
    });

    const unlistenLeft = onDragLeft((event) => {
      if (event.windowLabel === currentWindowLabel) {
        setIsHovered(false);
        options.onDragLeave?.();
      }
    });

    const unlistenEnded = onDragEnded((event) => {
      if (currentSessionRef.current === event.sessionId && isHovered) {
        if (event.result.type === 'Dropped') {
          options.onDrop?.(dragItems);
        }
      }
      setIsHovered(false);
      setDragItems([]);
      currentSessionRef.current = null;
    });

    return () => {
      unlistenEntered.then((fn) => fn());
      unlistenLeft.then((fn) => fn());
      unlistenEnded.then((fn) => fn());
    };
  }, [currentWindowLabel, options.onDrop, options.onDragEnter, options.onDragLeave, isHovered, dragItems]);

  const dropZoneProps = {
    'data-drop-zone': true,
    'data-hovered': isHovered,
  };

  return {
    isHovered,
    dragItems,
    dropZoneProps,
  };
}
