import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { SlidersHorizontal } from "@phosphor-icons/react";
import {
	Dialog,
	dialogManager,
	Input,
	Select,
	SelectOption,
	toast,
	useDialog,
	type UseDialogProps,
} from "@spacedrive/primitives";
import type { FileSetAttributesInput, SdPath } from "@sd/ts-client";
import {
	useLibraryMutation,
	useLibraryPreview,
	useLibraryValidate,
} from "../../contexts/SpacedriveContext";
import { hasErrors, pathName, PreflightPanel } from "./PreflightPanel";
import { describeError, Failure, useSubmitOnEnter } from "./RearrangeModal";

type Hidden = "keep" | "hidden" | "visible";

interface AttributesDialogProps {
	id: number;
	paths: SdPath[];
	onComplete?: () => void;
}

/**
 * Set what the filesystem lets a file carry: the permission mode, the
 * modification time, and whether it is hidden. Validation refuses what the
 * target filesystem cannot express, and the plan names what changes.
 */
export function useAttributesDialog() {
	return (options: Omit<AttributesDialogProps, "id">) =>
		dialogManager.create((props: UseDialogProps) => (
			<AttributesDialog {...(props as AttributesDialogProps)} {...options} />
		));
}

function AttributesDialog(props: AttributesDialogProps) {
	const dialog = useDialog(props);
	const form = useForm();
	const [mode, setMode] = useState("");
	const [modified, setModified] = useState("");
	const [hidden, setHidden] = useState<Hidden>("keep");
	const [failure, setFailure] = useState<string | null>(null);

	const parsedMode = /^[0-7]{3,4}$/.test(mode) ? parseInt(mode, 8) : null;
	const parsedModified = modified ? new Date(modified).getTime() : null;
	const anything =
		parsedMode !== null || (parsedModified !== null && !Number.isNaN(parsedModified)) || hidden !== "keep";

	const input = useMemo<FileSetAttributesInput>(
		() => ({
			paths: props.paths,
			attributes: {
				mode: parsedMode,
				modified_ms: parsedModified !== null && !Number.isNaN(parsedModified) ? parsedModified : null,
				hidden: hidden === "keep" ? null : hidden === "hidden",
			},
		}),
		[props.paths, parsedMode, parsedModified, hidden],
	);
	const validation = useLibraryValidate(
		{ type: "files.set_attributes", input },
		{ enabled: anything },
	);
	const preview = useLibraryPreview({ type: "files.set_attributes", input }, { enabled: anything });
	const set = useLibraryMutation("files.set_attributes");

	const refused = hasErrors(validation.data);
	const close = () => dialogManager.setState(props.id, { open: false });
	const submit = async () => {
		setFailure(null);
		try {
			await set.mutateAsync(input);
			toast.success(`Setting attributes on ${props.paths.length} items`);
			close();
			props.onComplete?.();
		} catch (error) {
			setFailure(describeError(error));
		}
	};
	useSubmitOnEnter(() => void submit(), anything && !refused && !set.isPending);

	return (
		<Dialog
			dialog={dialog}
			form={form}
			title={
				props.paths.length === 1
					? `Attributes of ${pathName(props.paths[0])}`
					: `Attributes of ${props.paths.length} items`
			}
			icon={<SlidersHorizontal size={20} weight="bold" />}
			ctaLabel="Apply"
			submitDisabled={!anything || refused || validation.isLoading || set.isPending}
			loading={set.isPending}
			onSubmit={form.handleSubmit(submit)}
			onCancelled={close}
			formClassName="!min-w-[480px] !max-w-[480px]"
		>
			<div className="space-y-4 py-1">
				<div className="text-sm text-ink-dull">
					Each field left empty stays as it is.
				</div>
				<label className="flex items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2 text-sm text-ink">
					<div>
						<div>Permission mode</div>
						<div className="text-[11px] text-ink-dull">Octal, such as 644 or 755.</div>
					</div>
					<Input
						size="xs"
						className="w-20"
						value={mode}
						placeholder="644"
						onChange={(event) => setMode(event.target.value.trim())}
					/>
				</label>
				<label className="flex items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2 text-sm text-ink">
					<div>Modified</div>
					<Input
						size="xs"
						type="datetime-local"
						className="w-52"
						value={modified}
						onChange={(event) => setModified(event.target.value)}
					/>
				</label>
				<label className="flex items-center justify-between gap-3 rounded-md bg-app-box px-3 py-2 text-sm text-ink">
					<div>Hidden</div>
					<Select<Hidden> value={hidden} size="sm" onChange={setHidden}>
						<SelectOption value="keep">as it is</SelectOption>
						<SelectOption value="hidden">hidden</SelectOption>
						<SelectOption value="visible">visible</SelectOption>
					</Select>
				</label>

				{anything && (
					<PreflightPanel
						validation={validation.data}
						validating={validation.isFetching}
						plan={preview.data}
						planning={preview.isFetching}
						planError={preview.error}
					/>
				)}
				{failure && <Failure message={failure} />}
			</div>
		</Dialog>
	);
}
