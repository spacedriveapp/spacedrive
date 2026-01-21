import { useMemo } from "react";
import {
  getCoreRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnSizingState,
} from "@tanstack/react-table";
import type { File } from "@sd/ts-client";

import { formatBytes, formatRelativeTime } from "../../utils";

export const ROW_HEIGHT = 36;
export const TABLE_PADDING_X = 16;
export const TABLE_PADDING_Y = 12;
export const TABLE_HEADER_HEIGHT = 32;

// Column definitions for the list view
export function useTable(files: File[]) {
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
    ],
    []
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