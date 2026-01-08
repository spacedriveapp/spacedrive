import { useLayoutEffect, useRef } from "react";
import { type TopBarPosition, useTopBar } from "./Context";
import { OverflowButton } from "./OverflowMenu";

interface TopBarSectionProps {
  position: TopBarPosition;
}

function ItemWrapper({
  id,
  children,
}: {
  id: string;
  children: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const { updateItemWidth } = useTopBar();
  const lastWidthRef = useRef<number>(0);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;

    const updateWidth = () => {
      const width = element.offsetWidth;
      if (width !== lastWidthRef.current) {
        lastWidthRef.current = width;
        updateItemWidth(id, width);
      }
    };

    // Initial measurement
    updateWidth();

    // Observe size changes
    const resizeObserver = new ResizeObserver(() => {
      updateWidth();
    });

    resizeObserver.observe(element);

    return () => {
      resizeObserver.disconnect();
    };
  }, [id, updateItemWidth]);

  return (
    <div className="inline-flex" ref={ref}>
      {children}
    </div>
  );
}

export function TopBarSection({ position }: TopBarSectionProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const {
    items,
    visibleItems,
    overflowItems,
    setLeftContainerRef,
    setRightContainerRef,
  } = useTopBar();

  useLayoutEffect(() => {
    if (position === "left") {
      setLeftContainerRef(containerRef);
    } else if (position === "right") {
      setRightContainerRef(containerRef);
    }
  }, [position, setLeftContainerRef, setRightContainerRef]);

  const positionItems = Array.from(items.values()).filter(
    (item) => item.position === position
  );

  const visible = positionItems.filter((item) => visibleItems.has(item.id));
  const overflow = overflowItems.get(position) || [];

  const containerClass =
    position === "center"
      ? "flex-1 flex items-center justify-center gap-2"
      : "flex items-center gap-2";

  return (
    <div className={containerClass} ref={containerRef}>
      {visible.map((item) => (
        <ItemWrapper id={item.id} key={item.id}>
          {item.element}
        </ItemWrapper>
      ))}
      {overflow.length > 0 && position !== "center" && (
        <OverflowButton items={overflow} />
      )}
    </div>
  );
}
