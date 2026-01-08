import { useEffect, useState } from "react";
import { type DragSession, getDragSession, onDragMoved } from "../lib/drag";

export function DragOverlay() {
  const [session, setSession] = useState<DragSession | null>(null);
  const [position, setPosition] = useState({ x: 0, y: 0 });

  useEffect(() => {
    // Get the session from query params
    const params = new URLSearchParams(window.location.search);
    const sessionId = params.get("session");

    if (sessionId) {
      getDragSession().then((s) => setSession(s));
    }

    const unlisten = onDragMoved((event) => {
      setPosition({ x: event.x, y: event.y });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (!session) {
    return null;
  }

  const itemCount = session.config.items.length;

  return (
    <div className="flex h-full w-full select-none items-center justify-center bg-transparent">
      <div className="rounded-lg border-2 border-blue-400 bg-blue-600 px-4 py-3 text-white shadow-2xl">
        <div className="flex items-center gap-3">
          <div className="text-2xl" />
          <div>
            <div className="font-semibold text-sm">
              {itemCount} {itemCount === 1 ? "file" : "files"}
            </div>
            <div className="text-blue-100 text-xs opacity-80">
              {session.config.items[0]?.kind.type === "file"
                ? session.config.items[0].kind.path.split("/").pop()
                : "Dragging..."}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
