// @ts-nocheck
import {
  type UseMutationOptions,
  type UseMutationResult,
  useMutation,
} from "@tanstack/react-query";
import type { CoreAction, LibraryAction } from "../generated/types";
import { WIRE_METHODS } from "../generated/types";
import { useSpacedriveClient } from "./useClient";

/**
 * Type-safe hook for core-scoped mutations
 *
 * @example
 * ```tsx
 * function CreateLibraryButton() {
 *   const createLib = useCoreMutation('libraries.create');
 *
 *   return (
 *     <button onClick={() => createLib.mutate({ name: 'My Library', path: null })}>
 *       Create Library
 *     </button>
 *   );
 * }
 * ```
 */
export function useCoreMutation<T extends CoreAction["type"]>(
  type: T,
  options?: Omit<
    UseMutationOptions<
      Extract<CoreAction, { type: T }>["output"],
      Error,
      Extract<CoreAction, { type: T }>["input"]
    >,
    "mutationFn"
  >
): UseMutationResult<
  Extract<CoreAction, { type: T }>["output"],
  Error,
  Extract<CoreAction, { type: T }>["input"]
> {
  const client = useSpacedriveClient();
  const wireMethod = WIRE_METHODS.coreActions[type]; // ← Auto-generated!

  return useMutation({
    mutationFn: (input) => client.execute(wireMethod, input),
    ...options,
  }) as UseMutationResult<
    Extract<CoreAction, { type: T }>["output"],
    Error,
    Extract<CoreAction, { type: T }>["input"]
  >;
}

/**
 * Type-safe hook for library-scoped mutations
 *
 * Automatically uses the current library ID from the client
 *
 * @example
 * ```tsx
 * function ApplyTagsButton({ entryIds, tagIds }: { entryIds: number[], tagIds: string[] }) {
 *   const applyTags = useLibraryMutation('tags.apply');
 *
 *   return (
 *     <button onClick={() => applyTags.mutate({ entry_ids: entryIds, tag_ids: tagIds })}>
 *       Apply Tags
 *     </button>
 *   );
 * }
 * ```
 */
export function useLibraryMutation<T extends LibraryAction["type"]>(
  type: T,
  options?: Omit<
    UseMutationOptions<
      Extract<LibraryAction, { type: T }>["output"],
      Error,
      Extract<LibraryAction, { type: T }>["input"]
    >,
    "mutationFn"
  >
): UseMutationResult<
  Extract<LibraryAction, { type: T }>["output"],
  Error,
  Extract<LibraryAction, { type: T }>["input"]
> {
  const client = useSpacedriveClient();
  const wireMethod = WIRE_METHODS.libraryActions[type]; // ← Auto-generated!

  return useMutation({
    mutationFn: (input) => {
      const libraryId = client.getCurrentLibraryId();
      if (!libraryId) {
        throw new Error(
          "No library selected. Use client.switchToLibrary() first."
        );
      }

      // Client.execute() automatically adds library_id to the request
      // as a sibling field (not inside payload)
      return client.execute(wireMethod, input);
    },
    ...options,
  }) as UseMutationResult<
    Extract<LibraryAction, { type: T }>["output"],
    Error,
    Extract<LibraryAction, { type: T }>["input"]
  >;
}
