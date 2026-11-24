import { CaretDown, CaretRight, FilmStrip, Image } from "@phosphor-icons/react";
import type { File, FileByPathQuery, SpokenTime, TextKind } from "@sd/ts-client";
import { WIRE_METHODS } from "@sd/ts-client";
import { toast } from "@spacedrive/primitives";
import clsx from "clsx";
import { useState } from "react";
import { useSpacedriveClient } from "../../../../contexts/SpacedriveContext";
import { useExplorer } from "../../context";
import {
	useContentSearch,
	type ContentFileHits,
	type ContentMoment,
} from "../../hooks/useContentSearch";
import { useSelection } from "../../SelectionContext";

/** Moments a file shows before it is expanded. */
const COLLAPSED_MOMENTS = 3;

const KIND_LABELS: Partial<Record<TextKind, string>> = {
	transcript: "Transcript",
	scene: "Scene",
	caption: "Caption",
};

function formatTimestamp(seconds: number): string {
	const total = Math.floor(seconds);
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	const secs = (total % 60).toString().padStart(2, "0");
	return hours > 0
		? hours + ":" + minutes.toString().padStart(2, "0") + ":" + secs
		: minutes + ":" + secs;
}

/**
 * Files whose transcripts, scene descriptions or captions match the search,
 * shown above the file results. A timestamp chip opens Quick Preview at the
 * moment the words were said or the shot starts.
 */
export function ContentHits() {
	const { files, isLoading, pendingFiles } = useContentSearch();
	const [collapsed, setCollapsed] = useState(false);
	const open = useOpenContentHit();

	if (files.length === 0) return null;

	return (
		<section className="border-app-line max-h-[45%] shrink-0 overflow-y-auto border-b px-4 py-3">
			<button
				onClick={() => setCollapsed((value) => !value)}
				className="text-ink-dull hover:text-ink flex items-center gap-1.5 text-xs font-semibold"
			>
				{collapsed ? (
					<CaretRight size={12} weight="bold" />
				) : (
					<CaretDown size={12} weight="bold" />
				)}
				In transcripts and descriptions
				<span className="text-ink-faint font-normal">
					{files.length}
					{isLoading ? "…" : ""}
				</span>
				{pendingFiles > 0 && (
					<span className="text-ink-faint font-normal">
						, {pendingFiles} not indexed yet
					</span>
				)}
			</button>
			{!collapsed && (
				<ul className="mt-2 space-y-1">
					{files.map((file) => (
						<ContentFileRow key={file.path} file={file} onOpen={open} />
					))}
				</ul>
			)}
		</section>
	);
}

function ContentFileRow({
	file,
	onOpen,
}: {
	file: ContentFileHits;
	onOpen: (file: ContentFileHits, time?: SpokenTime) => void;
}) {
	const [expanded, setExpanded] = useState(false);
	const moments = expanded
		? file.moments
		: file.moments.slice(0, COLLAPSED_MOMENTS);
	const hidden = file.moments.length - moments.length;
	const Icon = file.kinds.includes("caption") ? Image : FilmStrip;

	return (
		<li className="hover:bg-app-hover/50 rounded-md px-2 py-1.5">
			<div className="flex min-w-0 items-center gap-2">
				<Icon size={16} className="text-ink-dull shrink-0" />
				<button
					onClick={() => onOpen(file)}
					className="text-ink truncate text-sm font-medium hover:underline"
					title={file.path}
				>
					{file.name}
				</button>
				{file.folder && (
					<span className="text-ink-faint truncate text-xs">{file.folder}</span>
				)}
				<span className="ml-auto flex shrink-0 gap-1">
					{file.kinds.map((kind) => (
						<span
							key={kind}
							className="bg-app-box border-app-line text-ink-dull rounded border px-1.5 py-px text-[10px] font-medium"
						>
							{KIND_LABELS[kind] ?? kind}
						</span>
					))}
				</span>
			</div>
			<ul className="mt-1 space-y-0.5 pl-6">
				{moments.map((moment, index) => (
					<MomentRow
						key={index}
						moment={moment}
						onSeek={(time) => onOpen(file, time)}
					/>
				))}
			</ul>
			{hidden > 0 && (
				<button
					onClick={() => setExpanded(true)}
					className="text-accent mt-0.5 pl-6 text-xs hover:underline"
				>
					{hidden} more
				</button>
			)}
		</li>
	);
}

function MomentRow({
	moment,
	onSeek,
}: {
	moment: ContentMoment;
	onSeek: (time: SpokenTime) => void;
}) {
	const { time } = moment;
	return (
		<li className="flex items-baseline gap-2 text-xs">
			{time ? (
				<button
					onClick={() => onSeek(time)}
					title={"Play from " + formatTimestamp(time.start_seconds)}
					className="bg-accent/15 text-accent hover:bg-accent/25 shrink-0 rounded px-1.5 py-px font-mono text-[11px] tabular-nums"
				>
					{formatTimestamp(time.start_seconds)}
				</button>
			) : null}
			<span className="text-ink-dull line-clamp-2">
				{moment.snippet.map((fragment, index) => (
					<span
						key={index}
						className={clsx(fragment.matched && "text-ink font-semibold")}
					>
						{fragment.text}
					</span>
				))}
			</span>
		</li>
	);
}

/**
 * Resolves a hit to its File and opens Quick Preview on it. The selection is
 * cleared first, because the preview follows a single selected file and would
 * otherwise switch straight back to it.
 */
function useOpenContentHit() {
	const client = useSpacedriveClient();
	const { openQuickPreview } = useExplorer();
	const { clearSelection } = useSelection();

	return async (file: ContentFileHits, time?: SpokenTime) => {
		try {
			const resolved = await client.execute<FileByPathQuery, File>(
				WIRE_METHODS.libraryQueries["files.by_path"],
				{ path: file.path },
			);
			clearSelection();
			openQuickPreview(
				resolved.id,
				time ? { startSeconds: time.start_seconds } : undefined,
			);
		} catch (error) {
			toast.error("Could not open " + file.name + ": " + String(error));
		}
	};
}
