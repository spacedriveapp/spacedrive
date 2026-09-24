import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { ArrowRight, FolderOpen, FolderSimplePlus, Eye } from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	Switch,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type { FileMergeInput, MergeConflictPolicy, SdPath } from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

interface MergeFoldersDialogProps {
	id: number;
	/** The folders to merge, in order. */
	sources: SdPath[];
	/** The existing folder they merge into. */
	destination: SdPath;
	policy?: MergeConflictPolicy;
	consume?: boolean;
	/** Remove from the destination what no source holds: a mirror. */
	removeExtras?: boolean;
	onComplete?: () => void;
}

const POLICIES: { value: MergeConflictPolicy; label: string; hint: string }[] = [
	{ value: "skip", label: "Keep existing", hint: "a file that differs stays as it is" },
	{ value: "overwrite", label: "Overwrite", hint: "the incoming file replaces it" },
	{ value: "keep_both", label: "Keep both", hint: "the incoming file is written beside it, numbered" },
	{ value: "keep_newer", label: "Keep newer", hint: "the later modification time wins" },
];

/**
 * The merge dialog: validate and preview the merge as its options change,
 * gate confirm on an error finding, and dispatch the previewed input
 * unchanged.
 */
export function useMergeFoldersDialog() {
	return (options: Omit<MergeFoldersDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<MergeFoldersDialog {...(props as MergeFoldersDialogProps)} {...options} />
		));
}

function MergeFoldersDialog(props: MergeFoldersDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [policy, setPolicy] = useState<MergeConflictPolicy>(props.policy ?? "skip");
	const [consume, setConsume] = useState(props.consume ?? false);
	const [removeExtras, setRemoveExtras] = useState(props.removeExtras ?? false);
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileMergeInput>(
		() => ({
			sources: { paths: props.sources },
			destination: props.destination,
			on_conflict: policy,
			consume_sources: consume,
			remove_extras: removeExtras,
		}),
		[props.sources, props.destination, policy, consume, removeExtras],
	);

	const validation = useLibraryValidate({ type: "files.merge", input });
	const preview = useLibraryPreview({ type: "files.merge", input });
	const merge = useLibraryMutation("files.merge");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const close = () => dialogManager.setState(props.id, { open: false });

	const submit = async () => {
		setFailure(null);
		try {
			await merge.mutateAsync(input);
			toast.success(`Merging into ${pathName(props.destination)}`);
			close();
			props.onComplete?.();
		} catch (error) {
			if (error instanceof RefusedError) {
				setFailure(error.validation.findings.map((finding) => finding.message).join("; "));
			} else {
				setFailure(error instanceof Error ? error.message : String(error));
			}
		}
	};

	const browse = () => {
		if (!preview.data) return;
		enterPreview({
			source: { type: "files.merge", input },
			plan: preview.data,
			label: `Merge ${props.sources.map(pathName).join(", ")} into ${pathName(props.destination)}`,
		});
		close();
	};

	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			if ((event.target as HTMLElement)?.tagName === "INPUT") return;
			if (event.key === "Enter" && !event.shiftKey && !refused && !merge.isPending) {
				event.preventDefault();
				void submit();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	});

	const summary = preview.data?.summary;
	const cta = summary
		? `Merge ${summary.creates.files + summary.replaces.files} files${
				removeExtras && summary.deletes.files > 0 ? `, remove ${summary.deletes.files}` : ""
			}`
		: "Merge";

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title="Merge folders"
			icon={<FolderSimplePlus size={20} weight="bold" />}
			ctaLabel={cta}
			submitDisabled={refused || validation.isLoading || merge.isPending}
			loading={merge.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[520px] !max-w-[520px]"
		>
			<div className="space-y-4 py-1">
				<div className="flex items-center gap-3 text-sm">
					<div className="min-w-0 flex-1">
						<div className="text-xs text-ink-dull">Merge</div>
						{props.sources.map((source) => (
							<div key={JSON.stringify(source)} className="truncate font-medium text-ink">
								{pathName(source)}
							</div>
						))}
					</div>
					<ArrowRight className="size-5 flex-shrink-0 text-accent" weight="bold" />
					<div className="min-w-0 flex-1">
						<div className="text-xs text-ink-dull">Into</div>
						<div className="flex items-center gap-1.5 truncate font-medium text-ink">
							<FolderOpen className="size-4 flex-shrink-0 text-accent" weight="fill" />
							{pathName(props.destination)}
						</div>
					</div>
				</div>

				<div className="space-y-1">
					<div className="text-xs font-medium text-ink-dull">When a file differs at the same path</div>
					<div className="grid grid-cols-2 gap-1">
						{POLICIES.map((option) => (
							<button
								key={option.value}
								type="button"
								onClick={() => setPolicy(option.value)}
								className={
									policy === option.value
										? "rounded-md bg-accent px-2.5 py-1.5 text-left text-white"
										: "rounded-md bg-app-box px-2.5 py-1.5 text-left text-ink hover:bg-app-hover"
								}
							>
								<div className="text-sm font-medium">{option.label}</div>
								<div className="text-[11px] opacity-70">{option.hint}</div>
							</button>
						))}
					</div>
				</div>

				<label className="flex cursor-pointer items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2">
					<div>
						<div className="text-sm text-ink">Consume the sources</div>
						<div className="text-[11px] text-ink-dull">
							Remove each file once it is merged and prune emptied folders. What the merge
							does not settle stays.
						</div>
					</div>
					<Switch checked={consume} onCheckedChange={setConsume} size="sm" />
				</label>

				<label className="flex cursor-pointer items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2">
					<div>
						<div className="text-sm text-ink">Remove what the sources lack</div>
						<div className="text-[11px] text-ink-dull">
							The destination ends up matching the sources: files it holds that no source
							does go to the trash, and the plan flags the last copies among them.
						</div>
					</div>
					<Switch checked={removeExtras} onCheckedChange={setRemoveExtras} size="sm" />
				</label>

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{failure && (
					<div className="rounded-md border border-red-500/20 bg-red-500/10 px-2.5 py-2 text-xs text-red-500">
						{failure}
					</div>
				)}

				<div className="flex justify-end">
					<Button
						type="button"
						variant="outline"
						size="sm"
						disabled={!preview.data}
						onClick={browse}
					>
						<Eye className="mr-1.5 size-4" weight="bold" />
						Browse the result
					</Button>
				</div>
			</div>
		</Dialog>
	);
}
