import { useCallback, useEffect, useState } from 'react';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  beginDrag,
  endDrag,
  onDragBegan,
  onDragEnded,
  onDragMoved,
  type DragConfig,
  type DragSession,
  type DragResult,
  type DragMoveEvent,
} from '../lib/drag';

export interface UseDragOperationOptions {
  onDragStart?: (sessionId: string) => void;
  onDragMove?: (x: number, y: number) => void;
  onDragEnd?: (result: DragResult) => void;
}

export function useDragOperation(options: UseDragOperationOptions = {}) {
  const [isDragging, setIsDragging] = useState(false);
  const [currentSession, setCurrentSession] = useState<DragSession | null>(
    null
  );
  const [cursorPosition, setCursorPosition] = useState<{
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    const unlistenBegan = onDragBegan((event) => {
      setIsDragging(true);
      options.onDragStart?.(event.sessionId);
    });

    const unlistenMoved = onDragMoved((event: DragMoveEvent) => {
      setCursorPosition({ x: event.x, y: event.y });
      options.onDragMove?.(event.x, event.y);
    });

    const unlistenEnded = onDragEnded((event) => {
      setIsDragging(false);
      setCurrentSession(null);
      setCursorPosition(null);
      options.onDragEnd?.(event.result);
    });

    return () => {
      unlistenBegan.then((fn) => fn());
      unlistenMoved.then((fn) => fn());
      unlistenEnded.then((fn) => fn());
    };
  }, [options.onDragStart, options.onDragMove, options.onDragEnd]);

  const startDrag = useCallback(
    async (config: Omit<DragConfig, 'overlayUrl' | 'overlaySize'>) => {
      const currentWindow = getCurrentWebviewWindow();
      const sessionId = await beginDrag(
        {
          ...config,
          overlayUrl: '/drag-overlay',
          overlaySize: [200, 150],
        },
        currentWindow.label
      );
      return sessionId;
    },
    []
  );

  const cancelDrag = useCallback(async (sessionId: string) => {
    await endDrag(sessionId, { type: 'Cancelled' });
  }, []);

  return {
    isDragging,
    currentSession,
    cursorPosition,
    startDrag,
    cancelDrag,
  };
}
