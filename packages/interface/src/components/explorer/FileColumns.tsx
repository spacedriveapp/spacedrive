export interface IColumn {
	column: string;
	key: string;
	width: number;
}

export const LIST_VIEW_HEADER_HEIGHT = 40;

// Function ensure no types are lost, but guarantees that they are Column[]
export function ensureIsColumns<T extends IColumn[]>(data: T) {
	return data;
}

export const columns = [
	{ column: 'Name', key: 'name', width: 280 },
	// { column: 'Size', key: 'size_in_bytes', width: 120 },
	{ column: 'Type', key: 'extension', width: 150 },
	{ column: 'Size', key: 'size', width: 100 },
	{ column: 'Date Created', key: 'date_created', width: 150 },
	{ column: 'Content ID', key: 'cas_id', width: 150 }
] as const satisfies IColumn[];

export type ColumnKey = (typeof columns)[number]['key'];

export function ListViewHeader() {
	return (
		<div
			style={{ height: LIST_VIEW_HEADER_HEIGHT }}
			className="mr-2 flex w-full flex-row rounded-lg border-2 border-transparent"
		>
			{columns.map((col) => (
				<div
					key={col.key}
					className="flex items-center px-4 py-2 pr-2"
					style={{ width: col.width, marginTop: -LIST_VIEW_HEADER_HEIGHT * 2 }}
				>
					<span className="text-xs font-medium ">{col.column}</span>
				</div>
			))}
		</div>
	);
}
