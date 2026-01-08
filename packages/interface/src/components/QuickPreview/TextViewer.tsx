import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import clsx from "clsx";
import { memo, useEffect, useRef, useState } from "react";

import { languageMapping } from "./prism";

const prismaLazy = import("./prism-lazy");
prismaLazy.catch((e) => console.error("Failed to load prism-lazy", e));

export interface TextViewerProps {
  src: string;
  className?: string;
  onLoad?: (event: HTMLElementEventMap["load"]) => void;
  onError?: (event: HTMLElementEventMap["error"]) => void;
  codeExtension?: string;
  isSidebarPreview?: boolean;
}

export const TextViewer = memo(
  ({
    src,
    className,
    onLoad,
    onError,
    codeExtension,
    isSidebarPreview,
  }: TextViewerProps) => {
    const [lines, setLines] = useState<string[]>([]);
    const parentRef = useRef<HTMLPreElement>(null);
    const rowVirtualizer = useVirtualizer({
      count: lines.length,
      getScrollElement: () => parentRef.current,
      estimateSize: () => 22,
    });

    useEffect(() => {
      if (!src || src === "#") return;

      const controller = new AbortController();
      fetch(src, {
        mode: "cors",
        signal: controller.signal,
      })
        .then((response) => {
          if (!response.ok)
            throw new Error(`Invalid response: ${response.statusText}`);
          if (!response.body) return;
          onLoad?.(new UIEvent("load", {}));

          const reader = response.body
            .pipeThrough(new TextDecoderStream())
            .getReader();
          return reader.read().then(function ingestLines({
            done,
            value,
          }): void | Promise<void> {
            if (done) return;

            const chunks = value.split("\n");
            setLines([...chunks]);

            if (isSidebarPreview) return;

            return reader.read().then(ingestLines);
          });
        })
        .catch((error) => {
          if (!controller.signal.aborted)
            onError?.(new ErrorEvent("error", { message: `${error}` }));
        });

      return () => controller.abort();
    }, [src, onError, onLoad, codeExtension, isSidebarPreview]);

    return (
      <pre className={className} ref={parentRef} tabIndex={0}>
        <div
          className={clsx(
            "relative w-full whitespace-pre text-ink text-sm",
            codeExtension &&
              `language-${languageMapping.get(codeExtension) ?? codeExtension}`
          )}
          style={{
            height: `${rowVirtualizer.getTotalSize()}px`,
          }}
          tabIndex={0}
        >
          {rowVirtualizer.getVirtualItems().map((row) => (
            <TextRow
              codeExtension={codeExtension}
              content={lines[row.index]!}
              key={row.key}
              row={row}
            />
          ))}
        </div>
      </pre>
    );
  }
);

function TextRow({
  codeExtension,
  row,
  content,
}: {
  codeExtension?: string;
  row: VirtualItem;
  content: string;
}) {
  const contentRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const ref = contentRef.current;
    if (ref == null) return;

    let intersectionObserver: null | IntersectionObserver = null;

    prismaLazy.then(({ highlightElement }) => {
      intersectionObserver = new IntersectionObserver((events) => {
        for (const event of events) {
          if (
            !event.isIntersecting ||
            ref.getAttribute("data-highlighted") === "true"
          )
            continue;

          ref.setAttribute("data-highlighted", "true");
          highlightElement(event.target, false);

          const children = ref.children;
          if (children) {
            for (const elem of children) {
              elem.classList.remove("table");
            }
          }
        }
      });
      intersectionObserver.observe(ref);
    });

    return () => intersectionObserver?.disconnect();
  }, []);

  return (
    <div
      className={clsx("absolute top-0 left-0 flex w-full whitespace-pre")}
      style={{
        height: `${row.size}px`,
        transform: `translateY(${row.start}px)`,
      }}
    >
      {codeExtension && (
        <div
          className={clsx(
            "token block shrink-0 whitespace-pre pr-4 pl-2 text-gray-450 text-sm leading-6"
          )}
          key={row.key}
        >
          {row.index + 1}
        </div>
      )}
      <span className="flex-1 pl-2" ref={contentRef}>
        {content}
      </span>
    </div>
  );
}
