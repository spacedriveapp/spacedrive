interface Column {
	column: string;
	key: string;
	width: number;
}

export const columns = [
	{ column: 'Name', key: 'name', width: 280 },
	{ column: 'Type', key: 'extension', width: 150 },
	{ column: 'Size', key: 'size', width: 100 },
	{ column: 'Date Created', key: 'date_created', width: 150 },
	{ column: 'Content ID', key: 'cas_id', width: 150 }
] as const satisfies Readonly<Column[]>;

export const ROW_HEADER_HEIGHT = 40;

export const RowHeader = () => (
	<div
		style={{ height: ROW_HEADER_HEIGHT }}
		className="sticky mr-2 flex w-full flex-row rounded-lg border-2 border-transparent"
	>
		{columns.map((col) => (
			<div
				key={col.column}
				className="flex items-center px-4 py-2 pr-2"
				style={{ width: col.width, marginTop: -ROW_HEADER_HEIGHT * 2 }}
			>
				<span className="text-xs font-medium ">{col.column}</span>
			</div>
		))}
	</div>
);
