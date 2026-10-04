import { useEffect, useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { ArrowDown, ArrowUp, Eye, Plus, TextAa, X } from "@phosphor-icons/react";
import {
	Button,
	Dialog,
	dialogManager,
	Input,
	Select,
	SelectOption,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import clsx from "clsx";
import type { FileRenameBatchInput, RenameRule, SdPath } from "@sd/ts-client";
import { RefusedError } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { usePlanPreviewStore } from "../../routes/explorer/hooks/usePlanPreview";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";

interface RenameDialogProps {
	id: number;
	targets: SdPath[];
	onComplete?: () => void;
}

type RuleKind = RenameRule["kind"];
type CaseValue = Extract<RenameRule, { kind: "case" }>["stem"];

const KINDS: { value: RuleKind; label: string }[] = [
	{ value: "replace", label: "Replace text" },
	{ value: "case", label: "Change case" },
	{ value: "affix", label: "Add text" },
	{ value: "sequence", label: "Number" },
	{ value: "template", label: "Format" },
];

const TOKENS = "{name} {ext} {n} {n:04} {parent} {date:%Y-%m-%d} {captured:%Y-%m-%d}";

function blank(kind: RuleKind): RenameRule {
	switch (kind) {
		case "replace":
			return { kind, find: "", with: "", regex: false, whole_name: false };
		case "case":
			return { kind, stem: "lower", extension: "lower" };
		case "affix":
			return { kind, prefix: "", suffix: "" };
		case "sequence":
			return { kind, pattern: "{name} {n}", start: 1, step: 1 };
		case "template":
			return { kind, pattern: "{name}{ext}" };
	}
}

/**
 * Batch rename: an ordered list of rules applied to each name, previewed
 * as every rule changes. The plan is one move per changed name and a
 * conflict where two files want one, and an error finding, such as a name
 * the volume does not write or one already there, gates confirm.
 */
export function useBatchRenameDialog() {
	return (options: Omit<RenameDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<RenameDialog {...(props as RenameDialogProps)} {...options} />
		));
}

function RenameDialog(props: RenameDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [rules, setRules] = useState<RenameRule[]>([blank("replace")]);
	const [failure, setFailure] = useState<string | null>(null);

	const input = useMemo<FileRenameBatchInput>(
		() => ({ targets: props.targets, rules }),
		[props.targets, rules],
	);

	const validation = useLibraryValidate({ type: "files.rename_batch", input });
	const preview = useLibraryPreview({ type: "files.rename_batch", input });
	const rename = useLibraryMutation("files.rename_batch");
	const enterPreview = usePlanPreviewStore((state) => state.enter);

	const refused = hasErrors(validation.data);
	const summary = preview.data?.summary;
	const nothing = summary ? summary.moves.files === 0 : false;
	const close = () => dialogManager.setState(props.id, { open: false });

	const update = (index: number, rule: RenameRule) =>
		setRules((state) => state.map((held, at) => (at === index ? rule : held)));
	const remove = (index: number) =>
		setRules((state) => state.filter((_, at) => at !== index));
	const shift = (index: number, by: -1 | 1) =>
		setRules((state) => {
			const next = [...state];
			const other = index + by;
			if (other < 0 || other >= next.length) return state;
			[next[index], next[other]] = [next[other], next[index]];
			return next;
		});

	const submit = async () => {
		setFailure(null);
		try {
			await rename.mutateAsync(input);
			toast.success(`Renaming ${props.targets.length} items`);
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
			source: { type: "files.rename_batch", input },
			plan: preview.data,
			label: `Rename ${props.targets.length} items`,
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
				!rename.isPending
			) {
				event.preventDefault();
				void submit();
			}
		};
		window.addEventListener("keydown", onKey);
		return () => window.removeEventListener("keydown", onKey);
	});

	// Each old name beside its new one, and the names two files fight over.
	const renames = (preview.data?.changes ?? []).flatMap((change) => {
		if (change.change.type === "move") {
			return [{ from: pathName(change.change.from), to: pathName(change.path), conflict: false }];
		}
		if (change.change.type === "conflict") {
			return [{ from: "", to: pathName(change.path), conflict: true }];
		}
		return [];
	});

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={`Rename ${props.targets.length} items`}
			icon={<TextAa size={20} weight="bold" />}
			ctaLabel={summary ? `Rename ${summary.moves.files} files` : "Rename"}
			submitDisabled={refused || nothing || validation.isLoading || rename.isPending}
			loading={rename.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[600px] !max-w-[600px]"
		>
			<div className="space-y-4 py-1">
				<div className="space-y-2">
					{rules.map((rule, index) => (
						<RuleRow
							key={index}
							rule={rule}
							first={index === 0}
							last={index === rules.length - 1}
							onChange={(next) => update(index, next)}
							onRemove={() => remove(index)}
							onUp={() => shift(index, -1)}
							onDown={() => shift(index, 1)}
						/>
					))}
					<div className="flex items-center gap-2">
						<Select<RuleKind>
							value={"replace"}
							size="sm"
							onChange={(kind) => setRules((state) => [...state, blank(kind)])}
							placeholder="Add a rule"
						>
							{KINDS.map((kind) => (
								<SelectOption key={kind.value} value={kind.value}>
									<span className="flex items-center gap-1.5">
										<Plus className="size-3" weight="bold" />
										{kind.label}
									</span>
								</SelectOption>
							))}
						</Select>
						<span className="text-[10px] text-ink-faint">Tokens: {TOKENS}</span>
					</div>
				</div>

				{renames.length > 0 && (
					<div className="max-h-48 space-y-0.5 overflow-auto rounded-md bg-app-box p-2 text-[11px]">
						{renames.slice(0, 200).map((entry, index) => (
							<div
								key={`${entry.to}-${index}`}
								className={clsx(
									"flex items-center gap-2",
									entry.conflict && "text-red-500",
								)}
							>
								{entry.conflict ? (
									<span className="truncate">two files want {entry.to}</span>
								) : (
									<>
										<span className="truncate text-ink-dull">{entry.from}</span>
										<span className="flex-shrink-0 text-ink-faint">→</span>
										<span className="truncate text-ink">{entry.to}</span>
									</>
								)}
							</div>
						))}
						{renames.length > 200 && (
							<div className="text-ink-faint">and {renames.length - 200} more</div>
						)}
					</div>
				)}

				<PreflightPanel
					validation={validation.data}
					validating={validation.isFetching}
					plan={preview.data}
					planning={preview.isFetching}
					planError={preview.error}
				/>

				{nothing && (
					<div className="text-xs text-ink-faint">No name changes with these rules.</div>
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

function RuleRow({
	rule,
	first,
	last,
	onChange,
	onRemove,
	onUp,
	onDown,
}: {
	rule: RenameRule;
	first: boolean;
	last: boolean;
	onChange: (rule: RenameRule) => void;
	onRemove: () => void;
	onUp: () => void;
	onDown: () => void;
}) {
	return (
		<div className="flex items-center gap-2 rounded-md bg-app-box px-2 py-1.5">
			<span className="w-20 flex-shrink-0 text-xs font-medium text-ink">
				{KINDS.find((kind) => kind.value === rule.kind)?.label}
			</span>
			<div className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
				<RuleFields rule={rule} onChange={onChange} />
			</div>
			<button
				type="button"
				className="rounded p-0.5 text-ink-faint hover:text-ink disabled:opacity-30"
				disabled={first}
				onClick={onUp}
				title="Apply earlier"
			>
				<ArrowUp className="size-3.5" weight="bold" />
			</button>
			<button
				type="button"
				className="rounded p-0.5 text-ink-faint hover:text-ink disabled:opacity-30"
				disabled={last}
				onClick={onDown}
				title="Apply later"
			>
				<ArrowDown className="size-3.5" weight="bold" />
			</button>
			<button
				type="button"
				className="rounded p-0.5 text-ink-faint hover:text-red-500"
				onClick={onRemove}
				title="Remove the rule"
			>
				<X className="size-3.5" weight="bold" />
			</button>
		</div>
	);
}

function RuleFields({
	rule,
	onChange,
}: {
	rule: RenameRule;
	onChange: (rule: RenameRule) => void;
}) {
	switch (rule.kind) {
		case "replace":
			return (
				<>
					<Input
						size="xs"
						className="w-32"
						placeholder="find"
						value={rule.find}
						onChange={(event) => onChange({ ...rule, find: event.target.value })}
					/>
					<Input
						size="xs"
						className="w-32"
						placeholder="with"
						value={rule.with}
						onChange={(event) => onChange({ ...rule, with: event.target.value })}
					/>
					<Check
						label="regex"
						checked={rule.regex ?? false}
						onChange={(regex) => onChange({ ...rule, regex })}
					/>
					<Check
						label="whole name"
						checked={rule.whole_name ?? false}
						onChange={(whole_name) => onChange({ ...rule, whole_name })}
					/>
				</>
			);
		case "case":
			return (
				<>
					<Select<CaseValue>
						value={rule.stem}
						size="sm"
						onChange={(stem) => onChange({ ...rule, stem })}
					>
						<SelectOption value="lower">lowercase</SelectOption>
						<SelectOption value="upper">UPPERCASE</SelectOption>
						<SelectOption value="title">Title Case</SelectOption>
						<SelectOption value="keep">keep the stem</SelectOption>
					</Select>
					<Check
						label="lowercase extension"
						checked={rule.extension === "lower"}
						onChange={(lower) => onChange({ ...rule, extension: lower ? "lower" : "keep" })}
					/>
				</>
			);
		case "affix":
			return (
				<>
					<Input
						size="xs"
						className="w-32"
						placeholder="before"
						value={rule.prefix}
						onChange={(event) => onChange({ ...rule, prefix: event.target.value })}
					/>
					<Input
						size="xs"
						className="w-32"
						placeholder="after"
						value={rule.suffix}
						onChange={(event) => onChange({ ...rule, suffix: event.target.value })}
					/>
				</>
			);
		case "sequence":
			return (
				<>
					<Input
						size="xs"
						className="w-40"
						placeholder="{name} {n}"
						value={rule.pattern}
						onChange={(event) => onChange({ ...rule, pattern: event.target.value })}
					/>
					<Counter label="from" value={rule.start ?? 1} onChange={(start) => onChange({ ...rule, start })} />
					<Counter label="step" value={rule.step ?? 1} onChange={(step) => onChange({ ...rule, step })} />
				</>
			);
		case "template":
			return (
				<Input
					size="xs"
					className="w-64"
					placeholder="{name}{ext}"
					value={rule.pattern}
					onChange={(event) => onChange({ ...rule, pattern: event.target.value })}
				/>
			);
	}
}

function Check({
	label,
	checked,
	onChange,
}: {
	label: string;
	checked: boolean;
	onChange: (checked: boolean) => void;
}) {
	return (
		<label className="flex cursor-pointer items-center gap-1 text-[11px] text-ink-dull">
			<input
				type="checkbox"
				className="size-3 accent-accent"
				checked={checked}
				onChange={(event) => onChange(event.target.checked)}
			/>
			{label}
		</label>
	);
}

function Counter({
	label,
	value,
	onChange,
}: {
	label: string;
	value: number;
	onChange: (value: number) => void;
}) {
	return (
		<label className="flex items-center gap-1 text-[11px] text-ink-dull">
			{label}
			<Input
				size="xs"
				type="number"
				min={0}
				className="w-16"
				value={value}
				onChange={(event) => onChange(Math.max(0, Math.floor(Number(event.target.value) || 0)))}
			/>
		</label>
	);
}
