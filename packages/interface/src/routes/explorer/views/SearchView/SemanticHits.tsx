import { Sparkle } from "@phosphor-icons/react";
import { ContentFileRow, useOpenContentHit } from "./ContentHits";
import { useSemanticSearch } from "../../hooks/useSemanticSearch";

/**
 * Files about what the search describes, ranked by meaning and words
 * together. Shown above the file results while the Meaning mode is on.
 */
export function SemanticHits() {
	const { files, isLoading, pendingDocuments, error, failedSources } = useSemanticSearch(true);
	const open = useOpenContentHit();

	return (
		<section className="border-app-line max-h-[60%] shrink-0 overflow-y-auto border-b px-4 py-3">
			<div className="text-ink-dull flex items-center gap-1.5 text-xs font-semibold">
				<Sparkle size={12} weight="bold" />
				By meaning
				<span className="text-ink-faint font-normal">
					{isLoading ? "…" : files.length}
				</span>
				{pendingDocuments > 0 && (
					<span className="text-ink-faint font-normal">
						, {pendingDocuments} documents not embedded yet match by words only
					</span>
				)}
			</div>
			{error && (
				<p className="text-status-error mt-2 text-xs">
					{files.length > 0
						? `Search failed in ${failedSources} ${failedSources === 1 ? "source" : "sources"}: ${error}`
						: `Search by meaning failed: ${error}`}
				</p>
			)}
			{error && files.length === 0 ? null : !isLoading && files.length === 0 ? (
				<p className="text-ink-faint mt-2 text-xs">Nothing found by meaning.</p>
			) : (
				<ul className="mt-2 space-y-1">
					{files.map((file) => (
						<ContentFileRow key={file.path} file={file} onOpen={open} />
					))}
				</ul>
			)}
		</section>
	);
}
