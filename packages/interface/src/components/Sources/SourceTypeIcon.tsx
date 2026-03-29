interface SourceTypeIconProps {
	type: string;
	svg?: string | null;
	size?: "sm" | "md" | "lg";
}

const SIZE_CLASSES = {
	sm: "h-5 w-5 p-0.5",
	md: "h-8 w-8 p-1.5",
	lg: "h-10 w-10 p-2",
};

const FALLBACK_LABELS: Record<string, string> = {
	email: "mail",
	file: "doc",
	note: "note",
	bookmark: "link",
	history: "time",
	markdown: "md",
	session: "term",
};

export function SourceTypeIcon({
	type: typeName,
	svg,
	size = "md",
}: SourceTypeIconProps) {
	if (svg) {
		return (
			<div
				className={`shrink-0 rounded-lg [&>svg]:h-full [&>svg]:w-full ${SIZE_CLASSES[size]}`}
				dangerouslySetInnerHTML={{ __html: svg }}
			/>
		);
	}

	const label = FALLBACK_LABELS[typeName] ?? typeName.slice(0, 4);

	return (
		<div
			className={`bg-accent/10 text-accent flex shrink-0 items-center justify-center rounded-lg font-mono text-[10px] font-medium uppercase ${SIZE_CLASSES[size]}`}
		>
			{label}
		</div>
	);
}
