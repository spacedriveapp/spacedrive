/**
 * Duplicates: content this machine holds more than once, grouped by its
 * bytes, with a keeper chosen per group and one dialog to remove the rest.
 */

import { ArrowLeft, Stack } from "@phosphor-icons/react";
import { Button, CircleButton } from "@spacedrive/primitives";
import clsx from "clsx";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import type { DuplicateGroup } from "@sd/ts-client";
import { useDedupeDialog } from "../../components/modals/DedupeModal";
import { useCoreQuery } from "../../contexts/SpacedriveContext";
import { TopBarItem, TopBarPortal } from "../../TopBar";
import { formatBytes } from "../explorer/utils";

const SIZES: { label: string; bytes: number }[] = [
	{ label: "Any size", bytes: 0 },
	{ label: "1 MB+", bytes: 1024 * 1024 },
	{ label: "100 MB+", bytes: 100 * 1024 * 1024 },
	{ label: "1 GB+", bytes: 1024 * 1024 * 1024 },
];

export function DuplicatesView() {
	const navigate = useNavigate();
	const [minSize, setMinSize] = useState(SIZES[1].bytes);
	/** The chosen keeper per group, by content id; the first copy otherwise. */
	const [keepers, setKeepers] = useState<Record<string, string>>({});
	/** Groups left out of the removal, by content id. */
	const [excluded, setExcluded] = useState<Record<string, boolean>>({});
	const openDedupe = useDedupeDialog();

	const { data, isLoading, refetch } = useCoreQuery({
		type: "files.duplicates",
		input: { min_size: minSize, limit: 200, source: null },
	});
	const groups = data?.groups ?? [];

	const keeperOf = (group: DuplicateGroup) =>
		group.copies.find((copy) => copy.path === keepers[group.content]) ?? group.copies[0];

	const selected = useMemo(
		() => groups.filter((group) => !excluded[group.content]),
		[groups, excluded],
	);
	const reclaimable = selected.reduce((sum, group) => sum + group.reclaimable, 0);

	const remove = () => {
		if (selected.length === 0) return;
		openDedupe({
			title: `Remove the other copies of ${selected.length} ${
				selected.length === 1 ? "file" : "files"
			}`,
			targets: {
				kind: "duplicates",
				duplicates: {
					scope: null,
					keep: {
						kind: "these",
						paths: selected.map((group) => keeperOf(group).sd_path),
					},
					min_size: null,
				},
			},
			onComplete: () => void refetch(),
		});
	};

	const titleItem = (
		<div className="flex items-center gap-2">
			<CircleButton
				icon={ArrowLeft}
				title="Back to Protection"
				onClick={() => navigate("/redundancy")}
			/>
			<Stack size={18} weight="bold" className="text-ink" />
			<span className="text-sm font-semibold text-ink">Duplicates</span>
		</div>
	);

	return (
		<>
			<TopBarPortal
				center={
					<TopBarItem id="redundancy-duplicates-title" label="Duplicates" priority="high">
						{titleItem}
					</TopBarItem>
				}
			/>
			<div className="flex h-full flex-col overflow-hidden bg-app/80 pt-1.5">
				<div className="flex items-center gap-2 px-4 py-2">
					<div className="flex gap-1">
						{SIZES.map((size) => (
							<button
								key={size.bytes}
								type="button"
								onClick={() => setMinSize(size.bytes)}
								className={clsx(
									"rounded-md px-2.5 py-1 text-xs transition-colors",
									minSize === size.bytes
										? "bg-accent text-white"
										: "bg-app-box text-ink hover:bg-app-hover",
								)}
							>
								{size.label}
							</button>
						))}
					</div>
					<div className="flex-1 text-xs text-ink-dull">
						{data
							? `${groups.length} ${groups.length === 1 ? "group" : "groups"} across ${
									data.sources_queried
								} sources · ${formatBytes(reclaimable)} reclaimable in the selection`
							: isLoading
								? "Looking for copies…"
								: ""}
					</div>
					<Button
						type="button"
						variant="accent"
						size="sm"
						disabled={selected.length === 0}
						onClick={remove}
					>
						Remove other copies
					</Button>
				</div>

				<div className="flex-1 overflow-auto px-4 pb-4">
					{groups.length === 0 && !isLoading && (
						<div className="rounded-lg border border-app-line bg-app-box/50 p-6 text-center text-sm text-ink-dull">
							No content is held more than once at this size. A drive that is unplugged
							does not answer.
						</div>
					)}
					<div className="space-y-2">
						{groups.map((group) => {
							const keeper = keeperOf(group);
							const included = !excluded[group.content];
							return (
								<div
									key={group.content}
									className={clsx(
										"rounded-lg border border-app-line bg-app-box/50 p-3",
										!included && "opacity-50",
									)}
								>
									<div className="mb-2 flex items-center gap-3 text-xs">
										<label className="flex items-center gap-1.5 text-ink">
											<input
												type="checkbox"
												className="size-3.5 accent-accent"
												checked={included}
												onChange={(event) =>
													setExcluded((state) => ({
														...state,
														[group.content]: !event.target.checked,
													}))
												}
											/>
											{group.copies.length} copies of {formatBytes(group.size)}
										</label>
										<span className="text-ink-faint">
											{formatBytes(group.reclaimable)} reclaimable
										</span>
									</div>
									<div className="space-y-0.5">
										{group.copies.map((copy) => {
											const kept = copy.path === keeper.path;
											return (
												<label
													key={copy.path}
													className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-xs hover:bg-app-hover"
												>
													<input
														type="radio"
														name={`keep-${group.content}`}
														className="size-3.5 accent-accent"
														checked={kept}
														onChange={() =>
															setKeepers((state) => ({
																...state,
																[group.content]: copy.path,
															}))
														}
													/>
													<span
														className={clsx(
															"w-14 flex-shrink-0 font-medium",
															kept ? "text-accent" : "text-red-500",
														)}
													>
														{kept ? "keep" : "remove"}
													</span>
													<span
														className={clsx(
															"truncate",
															kept ? "text-ink" : "text-ink-dull line-through decoration-red-500/60",
														)}
														title={copy.path}
													>
														{copy.path}
													</span>
												</label>
											);
										})}
									</div>
								</div>
							);
						})}
					</div>
				</div>
			</div>
		</>
	);
}
