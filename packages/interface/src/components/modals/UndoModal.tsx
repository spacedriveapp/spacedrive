import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { ArrowCounterClockwise, Eye } from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type { FileUndoInput } from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

interface UndoDialogProps {
	id: number;
	/** The job whose journal is reversed. */
	job: string;
	/** What the job was, for the title. */
	label: string;
	/** Reverse only these effects, by sequence; the trash view restores one item this way. */
	effects?: number[];
	onComplete?: () => void;
}

/**
 * Undo on preflight: validation says what cannot be reversed and what
 * changed since the job ran, the plan is the reverse of the journal, and
 * confirm dispatches the same input.
 */
export function useUndoDialog() {
	return (options: Omit<UndoDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<UndoDialog {...(props as UndoDialogProps)} {...options} />
		));
}

function UndoDialog(props: UndoDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileUndoInput>(
		() => ({ job: props.job, effects: props.effects ?? null }),
		[props.job, props.effects],
	);

	const validation = useLibraryValidate({ type: "files.undo", input });
	const preview = useLibraryPreview({ type: "files.undo", input });
	const undo = useLibraryMutation("files.undo");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const close = () => dialogManager.setState(props.id, { open: false });

	const submit = async () => {
		setFailure(null);
		try {
			await undo.mutateAsync(input);
			toast.success(`Undoing ${props.label}`);
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
			source: { type: "files.undo", input },
			plan: preview.data,
			label: `Undo ${props.label}`,
		});
		close();
	};

	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			if ((event.target as HTMLElement)?.tagName === "INPUT") return;
			if (event.key === "Enter" && !event.shiftKey && !refused && !undo.isPending) {
				event.preventDefault();
				void submit();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	});

	// Each step of the reverse, as a person reads it.
	const steps = (preview.data?.changes ?? []).map((change) => {
		switch (change.change.type) {
			case "move":
				return `${pathName(change.change.from)} goes back to ${pathName(change.path)}`;
			case "delete":
				return `${pathName(change.path)} goes to the trash`;
			case "replace":
				return `${pathName(change.path)} gets its previous bytes back`;
			case "set_attributes":
				return `${pathName(change.path)} gets its attributes back`;
			default:
				return pathName(change.path);
		}
	});
	const count = steps.length;

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`Undo ${props.label}`}
			icon={<ArrowCounterClockwise size={20} weight="bold" />}
			ctaLabel={count > 0 ? `Reverse ${count} ${count === 1 ? "effect" : "effects"}` : "Undo"}
			submitDisabled={refused || validation.isLoading || undo.isPending}
			loading={undo.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[520px] !max-w-[520px]"
		>
			<div className="space-y-4 py-1">
				<div className="text-sm text-ink-dull">
					Reverses what the job did, newest effect first. A file that changed since is
					left as it is.
				</div>

				{steps.length > 0 && (
					<div className="max-h-48 space-y-0.5 overflow-auto rounded-md bg-app-box p-2 text-[11px] text-ink-dull">
						{steps.slice(0, 200).map((step, index) => (
							<div key={index} className="truncate">
								{step}
							</div>
						))}
						{steps.length > 200 && <div>and {steps.length - 200} more</div>}
					</div>
				)}

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
