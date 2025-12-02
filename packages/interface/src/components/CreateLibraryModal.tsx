import { useState, useEffect, useRef } from "react";
import { useForm } from "react-hook-form";
import {
	Books,
	FolderOpen,
	CircleNotch,
	CheckCircle,
	Warning,
} from "@phosphor-icons/react";
import {
	Button,
	Input,
	Label,
	Dialog,
	dialogManager,
	useDialog,
} from "@sd/ui";
import { queryClient } from "@sd/ts-client/hooks";
import type { Event } from "@sd/ts-client";
import { useCoreMutation, useSpacedriveClient } from "../context";
import { usePlatform } from "../platform";

interface CreateLibraryDialogProps {
	id: number;
	onLibraryCreated?: (libraryId: string) => void;
}

interface CreateLibraryFormData {
	name: string;
	path: string | null;
}

type DialogStep = "form" | "creating" | "success" | "error";

/**
 * Hook to open the Create Library dialog
 *
 * @example
 * ```tsx
 * const handleNewLibrary = () => {
 *   useCreateLibraryDialog((libraryId) => {
 *     console.log('Created library:', libraryId);
 *   });
 * };
 * ```
 */
export function useCreateLibraryDialog(
	onLibraryCreated?: (libraryId: string) => void,
) {
	return dialogManager.create((props: CreateLibraryDialogProps) => (
		<CreateLibraryDialog {...props} onLibraryCreated={onLibraryCreated} />
	));
}

function CreateLibraryDialog(props: CreateLibraryDialogProps) {
	const dialog = useDialog(props);
	const client = useSpacedriveClient();
	const platform = usePlatform();

	const [step, setStep] = useState<DialogStep>("form");
	const [errorMessage, setErrorMessage] = useState<string | null>(null);

	const createLibrary = useCoreMutation("libraries.create");

	// Track unsubscribe function and pending library ID in refs
	const unsubscribeRef = useRef<(() => void) | null>(null);
	const pendingLibraryIdRef = useRef<string | null>(null);
	// Buffer to store events received before we know the library ID
	const receivedEventsRef = useRef<Array<{ id: string; name: string; path: string }>>([]);

	const form = useForm<CreateLibraryFormData>({
		defaultValues: {
			name: "",
			path: null,
		},
	});

	// Clean up subscription on unmount
	useEffect(() => {
		return () => {
			if (unsubscribeRef.current) {
				unsubscribeRef.current();
				unsubscribeRef.current = null;
			}
		};
	}, []);

	const handleBrowse = async () => {
		if (!platform.openDirectoryPickerDialog) {
			console.error("Directory picker not available on this platform");
			return;
		}

		const selected = await platform.openDirectoryPickerDialog({
			title: "Choose library location",
			multiple: false,
		});

		if (selected && typeof selected === "string") {
			form.setValue("path", selected);
		}
	};

	const onSubmit = form.handleSubmit(async (data) => {
		if (!data.name.trim()) {
			form.setError("name", {
				type: "manual",
				message: "Library name is required",
			});
			return;
		}

		setStep("creating");
		setErrorMessage(null);
		receivedEventsRef.current = [];

		// Set up event subscription BEFORE making the mutation
		// This ensures we don't miss the LibraryCreated event
		try {
			const unsubscribe = await client.subscribe((event: Event) => {
				if (
					typeof event === "object" &&
					"LibraryCreated" in event
				) {
					const libraryEvent = event.LibraryCreated;

					// If we already know the library ID, check for match and close
					if (pendingLibraryIdRef.current === libraryEvent.id) {
						dialog.state.open = false;
						if (unsubscribeRef.current) {
							unsubscribeRef.current();
							unsubscribeRef.current = null;
						}
					} else {
						// Buffer the event in case it arrives before mutation resolves
						receivedEventsRef.current.push(libraryEvent);
					}
				}
			});
			unsubscribeRef.current = unsubscribe;
		} catch (err) {
			console.error("Failed to subscribe to events:", err);
		}

		try {
			const result = await createLibrary.mutateAsync({
				name: data.name.trim(),
				path: data.path,
			});

			// Store the library ID we're waiting for
			pendingLibraryIdRef.current = result.library_id;

			// Check if we already received the event (race condition handling)
			const alreadyReceived = receivedEventsRef.current.some(
				(e) => e.id === result.library_id
			);

			// Invalidate the libraries list query to refresh UI
			await queryClient.invalidateQueries({ queryKey: ["libraries"] });
			// Also invalidate core.status which includes library list
			await queryClient.invalidateQueries({ queryKey: ["core.status"] });

			// Switch to the new library
			if (platform.setCurrentLibraryId) {
				// Tauri: Use platform method to sync across all windows
				await platform.setCurrentLibraryId(result.library_id);
			} else {
				// Web fallback: Just update the client
				client.setCurrentLibrary(result.library_id);
			}

			// Call the callback if provided
			if (props.onLibraryCreated) {
				props.onLibraryCreated(result.library_id);
			}

			if (alreadyReceived) {
				// Event was already received, close immediately
				dialog.state.open = false;
				if (unsubscribeRef.current) {
					unsubscribeRef.current();
					unsubscribeRef.current = null;
				}
			} else {
				// Show success state while waiting for event
				setStep("success");
				// Dialog will close when LibraryCreated event is received
			}
		} catch (error) {
			console.error("Failed to create library:", error);
			setErrorMessage(
				error instanceof Error ? error.message : "Failed to create library",
			);
			setStep("error");

			// Clean up subscription on error
			if (unsubscribeRef.current) {
				unsubscribeRef.current();
				unsubscribeRef.current = null;
			}
		}
	});

	// Creating state
	if (step === "creating") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title="Creating Library"
				icon={<Books size={20} weight="fill" />}
				hideButtons
			>
				<div className="flex flex-col items-center justify-center py-8 gap-4">
					<CircleNotch
						className="size-12 text-accent animate-spin"
						weight="bold"
					/>
					<div className="text-center">
						<p className="text-sm font-medium text-ink">
							Creating your library...
						</p>
						<p className="text-xs text-ink-dull mt-1">
							This may take a moment
						</p>
					</div>
				</div>
			</Dialog>
		);
	}

	// Success state - waiting for LibraryCreated event
	if (step === "success") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title="Library Created"
				icon={<Books size={20} weight="fill" />}
				hideButtons
			>
				<div className="flex flex-col items-center justify-center py-8 gap-4">
					<CheckCircle
						className="size-12 text-green-500"
						weight="fill"
					/>
					<div className="text-center">
						<p className="text-sm font-medium text-ink">
							Library created successfully!
						</p>
						<p className="text-xs text-ink-dull mt-1">
							Initializing...
						</p>
					</div>
				</div>
			</Dialog>
		);
	}

	// Error state
	if (step === "error") {
		return (
			<Dialog
				dialog={dialog}
				form={form}
				title="Error"
				icon={<Warning size={20} weight="fill" className="text-red-500" />}
				ctaLabel="Try Again"
				onSubmit={async () => {
					setStep("form");
					setErrorMessage(null);
				}}
				onCancelled={true}
			>
				<div className="flex flex-col items-center justify-center py-6 gap-4">
					<Warning
						className="size-12 text-red-500"
						weight="fill"
					/>
					<div className="text-center">
						<p className="text-sm font-medium text-ink">
							Failed to create library
						</p>
						<p className="text-xs text-red-400 mt-1">
							{errorMessage}
						</p>
					</div>
				</div>
			</Dialog>
		);
	}

	// Form state (default)
	return (
		<Dialog
			dialog={dialog}
			form={form}
			onSubmit={onSubmit}
			title="Create New Library"
			icon={<Books size={20} weight="fill" />}
			description="A library is a container for your files, tags, and organization"
			ctaLabel="Create Library"
			onCancelled={true}
			loading={createLibrary.isPending}
		>
			<div className="space-y-4">
				<div className="space-y-2">
					<Label slug="name">Library Name</Label>
					<Input
						{...form.register("name", { required: "Name is required" })}
						size="md"
						placeholder="My Library"
						autoFocus
						className="bg-app-input"
					/>
					{form.formState.errors.name && (
						<p className="text-xs text-red-500">
							{form.formState.errors.name.message}
						</p>
					)}
				</div>

				<div className="space-y-2">
					<Label>
						Location{" "}
						<span className="text-ink-faint font-normal">(optional)</span>
					</Label>
					<div className="relative">
						<Input
							value={form.watch("path") || ""}
							onChange={(e) =>
								form.setValue("path", e.target.value || null)
							}
							size="md"
							placeholder="Default location"
							className="pr-12 bg-app-input"
						/>
						{platform.openDirectoryPickerDialog && (
							<Button
								type="button"
								variant="gray"
								size="sm"
								onClick={handleBrowse}
								className="absolute right-1.5 top-1/2 -translate-y-1/2"
							>
								<FolderOpen size={16} weight="bold" />
							</Button>
						)}
					</div>
					<p className="text-xs text-ink-faint">
						Leave empty to use the default location
					</p>
				</div>
			</div>
		</Dialog>
	);
}
