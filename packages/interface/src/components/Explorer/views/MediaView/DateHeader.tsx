import { useRef, useState, useEffect } from "react";
import clsx from "clsx";

export const DATE_HEADER_HEIGHT = 75;

interface DateHeaderProps {
  date?: string;
}

export function DateHeader({ date }: DateHeaderProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [isSticky, setIsSticky] = useState(false);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;

    const observer = new IntersectionObserver(
      ([entry]) => entry && setIsSticky(!entry.isIntersecting),
      { rootMargin: "-52px 0px 0px 0px", threshold: [1] },
    );

    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={ref}
      style={{ height: DATE_HEADER_HEIGHT }}
      className={clsx(
        "pointer-events-none sticky inset-x-0 -top-px z-10 p-5 transition-colors duration-500",
        !isSticky ? "text-ink" : "text-white",
      )}
    >
      <div
        className={clsx(
          "absolute inset-0 bg-gradient-to-b from-black/60 to-transparent transition-opacity duration-500",
          isSticky ? "opacity-100" : "opacity-0",
        )}
      />
      <div
        className={clsx(
          "relative text-xl font-semibold",
          !date && "opacity-75",
        )}
      >
        {date ?? "No date"}
      </div>
    </div>
  );
}
