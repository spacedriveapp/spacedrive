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

  const onDropRef = useRef(options.onDrop);
  const onDragEnterRef = useRef(options.onDragEnter);
  const onDragLeaveRef = useRef(options.onDragLeave);

  useEffect(() => {
    onDropRef.current = options.onDrop;
    onDragEnterRef.current = options.onDragEnter;
    onDragLeaveRef.current = options.onDragLeave;
  }, [options.onDrop, options.onDragEnter, options.onDragLeave]);

  useEffect(() => {
    const unlistenEntered = onDragEntered((event) => {
      if (event.windowLabel === currentWindowLabel) {
        setIsHovered(true);
        onDragEnterRef.current?.();
      }
    });

    const unlistenLeft = onDragLeft((event) => {
      if (event.windowLabel === currentWindowLabel) {
        setIsHovered(false);
        onDragLeaveRef.current?.();
      }
    });

    const unlistenEnded = onDragEnded((event) => {
      setIsHovered((prevHovered) => {
        setDragItems((prevItems) => {
          if (currentSessionRef.current === event.sessionId && prevHovered) {
            if (event.result.type === 'Dropped') {
              onDropRef.current?.(prevItems);
            }
          }
          return [];
        });
        currentSessionRef.current = null;
        return false;
      });
    });

    return () => {
      unlistenEntered.then((fn) => fn());
      unlistenLeft.then((fn) => fn());
      unlistenEnded.then((fn) => fn());
    };
  }, [currentWindowLabel]);

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
