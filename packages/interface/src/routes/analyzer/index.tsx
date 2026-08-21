import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { ArrowLeft, HardDrive, Plugs } from "@phosphor-icons/react";
import type { EphemeralSourceInfo, SizeTreeOutput } from "@sd/ts-client";
import { useCoreQuery } from "../../contexts/SpacedriveContext";
import { formatBytes } from "../overview/DevicePanel";
import { Sunburst } from "./Sunburst";

/**
 * The storage analyzer. `/analyzer` renders every indexed source as a small
 * sunburst in a scrolling grid; clicking a drive loads it in full fidelity.
 * The analyzer is a view over the index — sizes come from arena rollups via
 * files.size_tree, so drives answer even while unplugged.
 */
export function AnalyzerView() {
	const [params, setParams] = useSearchParams();
	const path = params.get("path");

	if (path) {
		return (
			<AnalyzerDetail
				path={path}
				onNavigate={(p) => setParams(p ? { path: p } : {})}
			/>
		);
	}
	return <AnalyzerGrid onOpen={(p) => setParams({ path: p })} />;
}

function AnalyzerGrid({ onOpen }: { onOpen: (path: string) => void }) {
	const { data: status } = useCoreQuery({
		type: "core.ephemeral_status",
		input: {},
	});
	const sources = (status?.sources ?? []) as EphemeralSourceInfo[];

	return (
		<div className="h-full overflow-y-auto p-8 pt-[52px]">
			<h1 className="text-xl font-semibold text-ink">Analyzer</h1>
			<p className="mt-1 text-sm text-ink-faint">
				Every indexed drive as a size breakdown — including drives that
				aren&apos;t plugged in.
			</p>

			{sources.length === 0 && (
				<div className="mt-16 text-center text-sm text-ink-faint">
					Nothing indexed yet — index a volume and it appears here.
				</div>
			)}

			<div className="mt-8 grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-4">
				{sources.map((source) => (
					<SourceCard
						key={source.id}
						source={source}
						onOpen={() => onOpen(source.root)}
					/>
				))}
			</div>
		</div>
	);
}

function SourceCard({
	source,
	onOpen,
}: {
	source: EphemeralSourceInfo;
	onOpen: () => void;
}) {
	const { data } = useCoreQuery({
		type: "files.size_tree",
		input: { path: source.root, depth: 2, top: 8 },
	});
	const tree = data as SizeTreeOutput | undefined;
	const name =
		source.root.split("/").filter(Boolean).pop() ?? source.root;

	return (
		<button
			onClick={onOpen}
			className={`group flex flex-col items-center rounded-lg border border-app-line/60 bg-app-box/40 p-4 text-left transition-colors hover:border-app-line hover:bg-app-box ${
				source.attached ? "" : "opacity-70"
			}`}
		>
			<div className="relative">
				{tree ? (
					<Sunburst root={tree.root} size={170} />
				) : (
					<div className="flex size-[170px] items-center justify-center">
						<HardDrive className="size-10 text-ink-faint/40" />
					</div>
				)}
			</div>
			<div className="mt-2 flex w-full items-center gap-1.5">
				<span className="min-w-0 flex-1 truncate text-sm font-medium text-ink">
					{name}
				</span>
				{!source.attached && (
					<span title="Not plugged in">
						<Plugs className="size-3.5 shrink-0 text-ink-faint" />
					</span>
				)}
			</div>
			<div className="w-full text-xs text-ink-faint">
				{source.total_bytes != null ? formatBytes(source.total_bytes) : "—"}
				{source.entry_count != null &&
					` · ${source.entry_count.toLocaleString()} items`}
			</div>
		</button>
	);
}

function AnalyzerDetail({
	path,
	onNavigate,
}: {
	path: string;
	onNavigate: (path: string | null) => void;
}) {
	const navigate = useNavigate();
	const [hovered, setHovered] = useState<{
		label: string;
		size: number;
	} | null>(null);

	const { data } = useCoreQuery({
		type: "files.size_tree",
		input: { path, depth: 3, top: 16 },
	});
	const tree = data as SizeTreeOutput | undefined;
	const root = tree?.root;

	const parent = path.split("/").slice(0, -1).join("/");

	return (
		<div className="flex h-full flex-col overflow-hidden p-8 pt-[52px]">
			<div className="flex items-center gap-3">
				<button
					onClick={() => onNavigate(null)}
					className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-ink-dull hover:bg-app-hover hover:text-ink"
				>
					<ArrowLeft className="size-4" /> All drives
				</button>
				{parent && (
					<button
						onClick={() => onNavigate(parent)}
						className="rounded-md px-2 py-1 text-sm text-ink-dull hover:bg-app-hover hover:text-ink"
					>
						Up
					</button>
				)}
				<span className="min-w-0 flex-1 truncate text-sm text-ink-faint">
					{path}
				</span>
			</div>

			<div className="flex min-h-0 flex-1 items-center justify-center gap-10">
				<div className="relative">
					{root && (
						<Sunburst
							root={root}
							size={440}
							onSelect={(p, isDir) => isDir && onNavigate(p)}
							onHover={setHovered}
						/>
					)}
					<div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
						<span className="max-w-[120px] truncate text-sm font-semibold text-ink">
							{hovered?.label ?? root?.name}
						</span>
						<span className="text-xs text-ink-faint">
							{formatBytes(hovered?.size ?? root?.size ?? 0)}
						</span>
					</div>
				</div>

				{root && (
					<div className="max-h-[440px] w-72 overflow-y-auto">
						{root.children.map((child) => (
							<button
								key={child.path}
								onClick={() =>
									child.is_dir
										? onNavigate(child.path)
										: navigate(
												`/explorer?path=${encodeURIComponent(
													JSON.stringify({
														Physical: {
															device_slug: "local",
															path: parent
																? child.path
																		.split("/")
																		.slice(0, -1)
																		.join("/")
																: child.path,
														},
													}),
												)}`,
											)
								}
								className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left hover:bg-app-hover"
							>
								<span className="min-w-0 flex-1 truncate text-sm text-ink-dull">
									{child.name}
								</span>
								<span className="shrink-0 text-xs tabular-nums text-ink-faint">
									{formatBytes(child.size)}
								</span>
							</button>
						))}
						{root.other_size > 0 && (
							<div className="flex w-full items-center gap-2 px-2 py-1">
								<span className="min-w-0 flex-1 truncate text-sm text-ink-faint">
									everything else
								</span>
								<span className="shrink-0 text-xs tabular-nums text-ink-faint">
									{formatBytes(root.other_size)}
								</span>
							</div>
						)}
					</div>
				)}
			</div>
		</div>
	);
}
