interface SourceStatusBadgeProps {
	status: string;
}

export function SourceStatusBadge({ status }: SourceStatusBadgeProps) {
	return (
		<span className="text-ink-faint inline-flex items-center gap-1.5 text-[11px] font-medium">
			<span
				className={`h-1.5 w-1.5 rounded-full ${
					status === "syncing"
						? "bg-accent animate-pulse"
						: status === "error"
							? "bg-red-400"
							: "bg-ink-faint"
				}`}
			/>
			{status}
		</span>
	);
}
