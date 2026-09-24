import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { Eye, FolderSimple, TreeStructure } from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	Select,
	SelectOption,
	Switch,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type {
	FileFlattenInput,
	FileOrganizeInput,
	FlattenPolicy,
	Granularity,
	OrganizeDateField,
	OrganizeRule,
	SdPath,
} from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

type RuleKind = OrganizeRule["kind"];

interface OrganizeDialogProps {
	id: number;
	scope: SdPath;
	onComplete?: () => void;
}

/**
 * Organize a folder: its files move into subfolders named by a rule, as
 * moves the plan shows before anything moves. Every move is a rename
 * inside the folder, so records keep their identity.
 */
export function useOrganizeDialog() {
	return (options: Omit<OrganizeDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<OrganizeDialog {...(props as OrganizeDialogProps)} {...options} />
		));
}

function OrganizeDialog(props: OrganizeDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [kind, setKind] = useState<RuleKind>("by_date");
	const [field, setField] = useState<OrganizeDateField>("modified");
	const [granularity, setGranularity] = useState<Granularity>("year_month");
	const [recursive, setRecursive] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileOrganizeInput>(() => {
		const rule: OrganizeRule =
			kind === "by_date"
				? { kind, field, granularity }
				: kind === "by_kind"
					? { kind }
					: { kind: "by_extension" };
		return { scope: props.scope, rule, recursive };
	}, [props.scope, kind, field, granularity, recursive]);

	const validation = useLibraryValidate({ type: "files.organize", input });
	const preview = useLibraryPreview({ type: "files.organize", input });
	const organize = useLibraryMutation("files.organize");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const summary = preview.data?.summary;
	const nothing = summary ? summary.moves.files === 0 : false;
	const close = () => dialogManager.setState(props.id, { open: false });

	const submit = async () => {
		setFailure(null);
		try {
			await organize.mutateAsync(input);
			toast.success(`Organizing ${pathName(props.scope)}`);
			close();
			props.onComplete?.();
		} catch (error) {
			setFailure(describeError(error));
		}
	};

	const browse = () => {
		if (!preview.data) return;
		enterPreview({
			source: { type: "files.organize", input },
			plan: preview.data,
			label: `Organize ${pathName(props.scope)}`,
		});
		close();
	};

	useSubmitOnEnter(() => void submit(), !refused && !nothing && !organize.isPending);

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`Organize ${pathName(props.scope)}`}
			icon={<TreeStructure size={20} weight="bold" />}
			ctaLabel={summary ? `Move ${summary.moves.files} files` : "Organize"}
			submitDisabled={refused || nothing || validation.isLoading || organize.isPending}
			loading={organize.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[520px] !max-w-[520px]"
		>
			<div className="space-y-4 py-1">
				<div className="space-y-2">
					<div className="text-xs font-medium text-ink-dull">Subfolders named by</div>
					<div className="flex flex-wrap items-center gap-2">
						<Select<RuleKind> value={kind} size="sm" onChange={setKind}>
							<SelectOption value="by_date">Date</SelectOption>
							<SelectOption value="by_kind">Kind</SelectOption>
							<SelectOption value="by_extension">Extension</SelectOption>
						</Select>
						{kind === "by_date" && (
							<>
								<Select<OrganizeDateField> value={field} size="sm" onChange={setField}>
									<SelectOption value="modified">modified</SelectOption>
									<SelectOption value="created">created</SelectOption>
									<SelectOption value="captured">captured</SelectOption>
								</Select>
								<Select<Granularity> value={granularity} size="sm" onChange={setGranularity}>
									<SelectOption value="year">2024</SelectOption>
									<SelectOption value="year_month">2024-05</SelectOption>
									<SelectOption value="year_month_day">2024-05-06</SelectOption>
								</Select>
							</>
						)}
					</div>
					{kind === "by_date" && field === "captured" && (
						<div className="text-[11px] text-ink-dull">
							The capture time from photos and videos, or the modification time where
							none is known.
						</div>
					)}
				</div>

				<label className="flex cursor-pointer items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2">
					<div>
						<div className="text-sm text-ink">Include files in subfolders</div>
						<div className="text-[11px] text-ink-dull">
							Every file beneath the folder, at any depth, ends up directly under its
							new subfolder.
						</div>
					</div>
					<Switch checked={recursive} onCheckedChange={setRecursive} size="sm" />
				</label>

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{nothing && (
					<div className="text-xs text-ink-faint">Nothing moves with this rule.</div>
				)}
				{failure && <Failure message={failure} />}
				<BrowseButton disabled={!preview.data} onClick={browse} />
			</div>
		</Dialog>
	);
}

interface FlattenDialogProps {
	id: number;
	scope: SdPath;
	onComplete?: () => void;
}

/**
 * Flatten a folder: every file beneath it moves up to the folder itself,
 * a name already taken numbered or left as the policy says, and the
 * emptied folders pruned.
 */
export function useFlattenDialog() {
	return (options: Omit<FlattenDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<FlattenDialog {...(props as FlattenDialogProps)} {...options} />
		));
}

function FlattenDialog(props: FlattenDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [policy, setPolicy] = useState<FlattenPolicy>("keep_both");
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileFlattenInput>(
		() => ({ scope: props.scope, on_conflict: policy }),
		[props.scope, policy],
	);
	const validation = useLibraryValidate({ type: "files.flatten", input });
	const preview = useLibraryPreview({ type: "files.flatten", input });
	const flatten = useLibraryMutation("files.flatten");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const summary = preview.data?.summary;
	const nothing = summary ? summary.moves.files === 0 : false;
	const close = () => dialogManager.setState(props.id, { open: false });

	const submit = async () => {
		setFailure(null);
		try {
			await flatten.mutateAsync(input);
			toast.success(`Flattening ${pathName(props.scope)}`);
			close();
			props.onComplete?.();
		} catch (error) {
			setFailure(describeError(error));
		}
	};

	const browse = () => {
		if (!preview.data) return;
		enterPreview({
			source: { type: "files.flatten", input },
			plan: preview.data,
			label: `Flatten ${pathName(props.scope)}`,
		});
		close();
	};

	useSubmitOnEnter(() => void submit(), !refused && !nothing && !flatten.isPending);

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`Flatten ${pathName(props.scope)}`}
			icon={<FolderSimple size={20} weight="bold" />}
			ctaLabel={summary ? `Move ${summary.moves.files} files up` : "Flatten"}
			submitDisabled={refused || nothing || validation.isLoading || flatten.isPending}
			loading={flatten.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[520px] !max-w-[520px]"
		>
			<div className="space-y-4 py-1">
				<div className="text-sm text-ink-dull">
					Every file beneath the folder moves up to it, and the folders left empty are
					removed.
				</div>
				<div className="space-y-1">
					<div className="text-xs font-medium text-ink-dull">When a name is already taken</div>
					<div className="grid grid-cols-2 gap-1">
						{(
							[
								{ value: "keep_both", label: "Keep both", hint: "the deeper file is numbered" },
								{ value: "skip", label: "Leave it", hint: "the deeper file stays where it is" },
							] as { value: FlattenPolicy; label: string; hint: string }[]
						).map((option) => (
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

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{nothing && (
					<div className="text-xs text-ink-faint">Nothing beneath the folder to move up.</div>
				)}
				{failure && <Failure message={failure} />}
				<BrowseButton disabled={!preview.data} onClick={browse} />
			</div>
		</Dialog>
	);
}

export function describeError(error: unknown): string {
	if (error instanceof RefusedError) {
		return error.validation.findings.map((finding) => finding.message).join("; ");
	}
	return error instanceof Error ? error.message : String(error);
}

/** Enter submits, outside inputs, while the action is allowed. */
export function useSubmitOnEnter(submit: () => void, allowed: boolean) {
	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			const tag = (event.target as HTMLElement)?.tagName;
			if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
			if (event.key === "Enter" && !event.shiftKey && allowed) {
				event.preventDefault();
				submit();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	});
}

export function Failure({ message }: { message: string }) {
	return (
		<div className="rounded-md border border-red-500/20 bg-red-500/10 px-2.5 py-2 text-xs text-red-500">
			{message}
		</div>
	);
}

export function BrowseButton({ disabled, onClick }: { disabled: boolean; onClick: () => void }) {
	return (
		<div className="flex justify-end">
			<Button type="button" variant="outline" size="sm" disabled={disabled} onClick={onClick}>
				<Eye className="mr-1.5 size-4" weight="bold" />
				Browse the result
			</Button>
		</div>
	);
}
