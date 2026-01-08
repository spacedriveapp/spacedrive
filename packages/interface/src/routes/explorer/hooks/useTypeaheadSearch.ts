import type { File } from "@sd/ts-client";
import { useCallback, useRef } from "react";

interface UseTypeaheadSearchProps {
  files: File[];
  onMatch: (file: File, index: number) => void;
  enabled?: boolean;
}

/**
 * Reusable typeahead search hook for file lists
 * Allows quick navigation by typing file names
 */
export function useTypeaheadSearch({
  files,
  onMatch,
  enabled = true,
}: UseTypeaheadSearchProps) {
  const searchStringRef = useRef("");
  const searchTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const handleKey = useCallback(
    (e: KeyboardEvent) => {
      if (!enabled) return false;

      // Only handle single character keys (typeahead)
      // No modifiers except Shift (for capitals)
      // Skip if target is an input element
      const target = e.target as HTMLElement;
      const isInputElement =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable;

      if (
        !isInputElement &&
        e.key.length === 1 &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        files.length > 0
      ) {
        // Clear previous timeout
        if (searchTimeoutRef.current) {
          clearTimeout(searchTimeoutRef.current);
        }

        // Update search string
        searchStringRef.current += e.key.toLowerCase();

        // Find first file that matches the search string
        const matchIndex = files.findIndex((file) => {
          const fileName = file.name.toLowerCase();
          return fileName.startsWith(searchStringRef.current);
        });

        // If match found, call the callback
        if (matchIndex !== -1) {
          onMatch(files[matchIndex], matchIndex);

          // Scroll to the matched file
          const element = document.querySelector(
            `[data-file-id="${files[matchIndex].id}"]`
          );
          if (element) {
            element.scrollIntoView({ block: "nearest", behavior: "smooth" });
          }
        }

        // Reset search string after 500ms of inactivity
        searchTimeoutRef.current = setTimeout(() => {
          searchStringRef.current = "";
        }, 500);

        return true; // Handled
      }

      return false; // Not handled
    },
    [files, onMatch, enabled]
  );

  // Cleanup timeout on unmount
  const cleanup = useCallback(() => {
    if (searchTimeoutRef.current) {
      clearTimeout(searchTimeoutRef.current);
    }
  }, []);

  return { handleKey, cleanup };
}
