import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { Eye, Stack } from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	Switch,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type { DeleteTargets, FileDeleteInput } from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { formatBytes } from "../../routes/explorer/utils";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

interface DedupeDialogProps {
	id: number;
	/** What goes: the surplus copies of duplicates, or what a folder already holds. */
	targets: DeleteTargets;
	title: string;
	onComplete?: () => void;
}

/**
 * Remove duplicate copies through delete's preflight: the plan lists the
 * copy of each content that stays and the copies that go, an error finding
 * gates confirm, and the job reads each pair in full before removing one.
 */
export function useDedupeDialog() {
	return (options: Omit<DedupeDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<DedupeDialog {...(props as DedupeDialogProps)} {...options} />
		));
}

function describe(targets: DeleteTargets): string {
	switch (targets.kind) {
		case "duplicates": {
			const { scope, keep } = targets.duplicates;
			if (keep.kind === "first") {
				return `Of each content held more than once under ${
					scope ? pathName(scope) : "the library"
				}, the first copy stays and the others go.`;
			}
			return `${keep.paths.length === 1 ? "The chosen copy stays" : `${keep.paths.length} chosen copies stay`}; every other copy of ${
				keep.paths.length === 1 ? "its" : "their"
			} content${scope ? ` under ${pathName(scope)}` : ""} goes.`;
		}
		case "comparison":
			return `Removes from ${pathName(targets.comparison.a)} the files whose bytes ${pathName(
				targets.comparison.b,
			)} already holds, wherever they sit in it.`;
		case "paths":
			return `Removes ${targets.paths.length} files.`;
	}
}

function DedupeDialog(props: DedupeDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [permanent, setPermanent] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileDeleteInput>(
		() => ({ targets: props.targets, permanent, recursive: true }),
		[props.targets, permanent],
	);

	const validation = useLibraryValidate({ type: "files.delete", input });
	const preview = useLibraryPreview({ type: "files.delete", input });
	const remove = useLibraryMutation("files.delete");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const summary = preview.data?.summary;
	const nothing = summary ? summary.deletes.files === 0 : false;
	const close = () => dialogManager.setState(props.id, { open: false });

	const submit = async () => {
		setFailure(null);
		try {
			await remove.mutateAsync(input);
			toast.success(props.title);
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
			source: { type: "files.delete", input },
			plan: preview.data,
			label: props.title,
		});
		close();
	};

	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			if ((event.target as HTMLElement)?.tagName === "INPUT") return;
			if (
				event.key === "Enter" &&
				!event.shiftKey &&
				!refused &&
				!nothing &&
				!remove.isPending
			) {
				event.preventDefault();
				void submit();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	});

	const cta = summary
		? `Remove ${summary.deletes.files} ${summary.deletes.files === 1 ? "copy" : "copies"} (${formatBytes(
				summary.deletes.bytes,
			)})`
		: "Remove";

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={props.title}
			icon={<Stack size={20} weight="bold" />}
			ctaLabel={cta}
			ctaDanger
			submitDisabled={refused || nothing || validation.isLoading || remove.isPending}
			loading={remove.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[520px] !max-w-[520px]"
		>
			<div className="space-y-4 py-1">
				<div className="text-sm text-ink-dull">{describe(props.targets)}</div>

				<label className="flex cursor-pointer items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2">
					<div>
						<div className="text-sm text-ink">Delete permanently</div>
						<div className="text-[11px] text-ink-dull">
							Off, the copies go to the trash.
						</div>
					</div>
					<Switch checked={permanent} onCheckedChange={setPermanent} size="sm" />
				</label>

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{nothing && (
					<div className="text-xs text-ink-faint">Nothing to remove: no surplus copies.</div>
				)}

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
