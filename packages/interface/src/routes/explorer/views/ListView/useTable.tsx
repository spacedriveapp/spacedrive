import { useMemo } from "react";
import {
  getCoreRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnSizingState,
} from "@tanstack/react-table";
import type { File } from "@sd/ts-client";

import { formatBytes, formatRelativeTime } from "../../utils";

/** The optional video columns the view settings turn on. */
export interface VideoColumns {
  duration: boolean;
  shots: boolean;
  transcript: boolean;
}

/** Ids of the columns whose cells read the listing's footage summaries. */
export const FOOTAGE_COLUMN_IDS = new Set(["duration", "shots", "transcript"]);

export const ROW_HEIGHT = 36;
export const TABLE_PADDING_X = 16;
export const TABLE_PADDING_Y = 12;
export const TABLE_HEADER_HEIGHT = 32;

// Column definitions for the list view
export function useTable(files: File[], video: VideoColumns) {
  // Memoize files array reference to prevent unnecessary table updates
  const stableFiles = useMemo(() => files, [JSON.stringify(files.map(f => f.id))]);

  const columns = useMemo<ColumnDef<File>[]>(
    () => [
      {
        id: "name",
        header: "Name",
        minSize: 200,
        size: 300,
        maxSize: 800,
        accessorFn: (row) => row.name,
      },
      {
        id: "size",
        header: "Size",
        size: 80,
        minSize: 60,
        maxSize: 120,
        accessorFn: (row) => (row.size > 0 ? formatBytes(row.size) : "—"),
      },
      {
        id: "modified",
        header: "Modified",
        size: 120,
        minSize: 80,
        maxSize: 180,
        accessorFn: (row) => formatRelativeTime(row.modified_at),
      },
      {
        id: "type",
        header: "Type",
        size: 80,
        minSize: 60,
        maxSize: 120,
        accessorFn: (row) =>
          row.kind === "File" ? row.extension?.toUpperCase() || "—" : "Folder",
      },
      ...(video.duration
        ? [
            { id: "duration", header: "Duration", size: 80, minSize: 60, maxSize: 120 },
          ]
        : []),
      ...(video.shots
        ? [{ id: "shots", header: "Shots", size: 64, minSize: 50, maxSize: 100 }]
        : []),
      ...(video.transcript
        ? [{ id: "transcript", header: "Transcript", size: 84, minSize: 60, maxSize: 120 }]
        : []),
    ],
    [video.duration, video.shots, video.transcript]
  );

  const coreRowModel = useMemo(() => getCoreRowModel<File>(), []);

  const table = useReactTable({
    data: stableFiles,
    columns,
    defaultColumn: {
      minSize: 60,
      maxSize: 500,
    },
    getCoreRowModel: coreRowModel,
    columnResizeMode: "onChange",
    getRowId: (row) => row.id,
  });

  return { table, columns };
}

export type { ColumnSizingState };