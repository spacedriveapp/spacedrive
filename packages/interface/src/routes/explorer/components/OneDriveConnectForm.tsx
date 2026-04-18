import { useCallback, useEffect, useMemo, useState } from "react";
import type { UseFormReturn } from "react-hook-form";
import { useWatch } from "react-hook-form";
import { CaretRight, Copy, ArrowSquareOut, X } from "@phosphor-icons/react";
import { Button, Input, Label } from "@spacedrive/primitives";
import type {
	CancelInput,
	OauthFlowStatus,
	PollInput,
	PollOutput,
	StartInput,
	StartOutput,
} from "@sd/ts-client";
import clsx from "clsx";

import {
	useLibraryMutation,
	useLibraryQuery,
} from "../../../contexts/SpacedriveContext";
import { usePlatform } from "../../../contexts/PlatformContext";

/**
 * Props for {@link OneDriveConnectForm}.
 *
 * Option A integration: the form receives the parent's react-hook-form
 * instance so successful OAuth writes `client_id` / `client_secret` /
 * `access_token` / `refresh_token` / `display_name` back into the shared
 * `CloudFormData` before delegating to the parent's existing
 * `onSubmitCloud` handler. That handler already wires up
 * `volumes.add_cloud` + location creation + modal close + query
 * invalidation, so we never duplicate that logic here.
 */
interface OneDriveConnectFormProps {
	/** Parent `useForm` for the shared cloud form data. */
	cloudForm: UseFormReturn<CloudFormShape>;
	/** Submit callback produced by `cloudForm.handleSubmit(...)` in the parent. */
	onSubmitCloud: (e?: React.BaseSyntheticEvent) => Promise<void>;
	/** Pending state for the downstream `volumes.add_cloud` mutation (shown on the submit button). */
	isAddingVolume: boolean;
}

/**
 * Shape of the parent's `CloudFormData` fields this component reads/writes.
 *
 * Declared as a structural subset so the file compiles standalone even if
 * `CloudFormData` gains fields later. The parent passes its full form via
 * a cast to `UseFormReturn<CloudFormShape>`; react-hook-form does not
 * narrow field access, so the extra fields remain available on its side.
 */
export interface CloudFormShape {
	display_name: string;
	client_id?: string;
	client_secret?: string;
	access_token?: string;
	refresh_token?: string;
	root?: string;
}

/**
 * Microsoft requires exact-match loopback redirect URIs for personal
 * accounts. Sharing the list with the backend's provider definition via a
 * constant is impractical (no cross-language source of truth for the five
 * ports), so we hardcode the same set the Rust provider uses and surface
 * them for the user to paste into the Azure portal.
 *
 * Keep in sync with `core/src/ops/cloud/oauth/providers/onedrive.rs`.
 */
export const ONEDRIVE_LOOPBACK_REDIRECT_URIS: readonly string[] = [
	"http://127.0.0.1:53682",
	"http://127.0.0.1:53683",
	"http://127.0.0.1:53684",
	"http://127.0.0.1:53685",
	"http://127.0.0.1:53686",
];

/**
 * Direct deep-link into the Azure portal's "App registrations" blade. This
 * skips the Entra ID landing page and drops the user exactly where step 1
 * of the tutorial tells them to go.
 */
export const AZURE_PORTAL_APP_REGISTRATIONS_URL =
	"https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade";

/**
 * Canonical UUID v4 regex. Azure always issues v4 client ids; a typo or a
 * stray whitespace character is the most common failure mode so we
 * validate upfront instead of waiting for the backend to reject the token
 * exchange.
 */
const UUID_V4_REGEX =
	/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/**
 * Returns `true` when `value` is a well-formed UUID v4. Exported for the
 * sibling test module.
 */
export function isValidClientId(value: string | undefined): boolean {
	if (!value) return false;
	return UUID_V4_REGEX.test(value.trim());
}

/**
 * Joins the five Microsoft redirect URIs with real newlines for the
 * "Copy redirect URIs" button. Kept pure so the parent's integration
 * test (if any) can verify the exact clipboard payload.
 */
export function buildRedirectUrisClipboardPayload(): string {
	return ONEDRIVE_LOOPBACK_REDIRECT_URIS.join("\n");
}

/**
 * Maps an {@link OauthFlowStatus} to a short, user-facing message.
 * `completed` is handled inline by the connected-callback path so it is
 * intentionally not surfaced here.
 */
export function statusToMessage(status: OauthFlowStatus | undefined): string | null {
	if (!status) return null;
	switch (status.type) {
		case "pending":
			return "Waiting for Microsoft sign-in…";
		case "failed":
			return status.error || "Sign-in failed. Try again.";
		case "cancelled":
			return "Sign-in cancelled.";
		case "completed":
			return null;
	}
}

type LocalError = { kind: "start" | "poll" | "generic"; message: string };

interface TutorialStep {
	title: string;
	body: string;
	caption: string;
}

const TUTORIAL_STEPS: readonly TutorialStep[] = [
	{
		title: "Open the Azure portal",
		body: "Click the button below to open Microsoft Entra ID's App registrations page. Sign in with the personal Microsoft account you want to connect.",
		caption: "App registrations landing page",
	},
	{
		title: "Create a new registration",
		body: "Click New registration. Pick any name (for example, Spacedrive). Under Supported account types, choose Personal Microsoft accounts only.",
		caption: "New registration form",
	},
	{
		title: "Add loopback redirect URIs",
		body: "Open the Authentication tab, add a platform of type Mobile and desktop applications, and paste the five loopback URIs using the button below.",
		caption: "Redirect URI editor",
	},
	{
		title: "Allow public client flows",
		body: "Still under Authentication, scroll to Advanced settings and set Allow public client flows to Yes. Save.",
		caption: "Public client flows toggle",
	},
	{
		title: "Grant API permissions",
		body: "Open API permissions, click Add a permission, pick Microsoft Graph, then Delegated permissions, and add Files.ReadWrite.All, offline_access, and User.Read.",
		caption: "API permissions list",
	},
	{
		title: "Copy the client id",
		body: "Go back to Overview and copy the Application (client) ID. Paste it into the Client ID field below.",
		caption: "Overview page with client id highlighted",
	},
	{
		title: "Create a client secret",
		body: "Open Certificates & secrets, click New client secret, copy the Value (not the Secret ID) immediately, and paste it into the Client secret field below. Microsoft only shows the value once.",
		caption: "Client secret creation screen",
	},
];

/**
 * Self-contained OneDrive OAuth connect form.
 *
 * High-level flow:
 * 1. User pastes BYO Azure AD `client_id` + `client_secret`.
 * 2. Clicking "Connect with Microsoft" calls `cloud.oauth.start`, opens
 *    the returned `auth_url` via `platform.openLink`, and stores the
 *    returned `flow_id` in local state.
 * 3. `useLibraryQuery({ type: "cloud.oauth.poll", input: { flow_id } })`
 *    with `refetchInterval: 1000` tracks progress.
 * 4. On `completed`, the received `TokenSet` is merged into the shared
 *    `cloudForm` and `onSubmitCloud()` is invoked so the existing
 *    `volumes.add_cloud` + location-creation pipeline handles the rest.
 * 5. On `failed` / `cancelled`, the error is shown and the form resets.
 * 6. A cancel button (or component unmount) sends `cloud.oauth.cancel`.
 */
export function OneDriveConnectForm({
	cloudForm,
	onSubmitCloud,
	isAddingVolume,
}: OneDriveConnectFormProps) {
	const platform = usePlatform();

	const [tutorialOpen, setTutorialOpen] = useState<boolean>(false);
	const [flowId, setFlowId] = useState<string | null>(null);
	const [redirectUri, setRedirectUri] = useState<string | null>(null);
	const [authUrl, setAuthUrl] = useState<string | null>(null);
	const [copiedRedirects, setCopiedRedirects] = useState<boolean>(false);
	const [error, setError] = useState<LocalError | null>(null);

	const clientId =
		useWatch({ control: cloudForm.control, name: "client_id" }) ?? "";
	const clientSecret =
		useWatch({ control: cloudForm.control, name: "client_secret" }) ?? "";
	const displayName =
		useWatch({ control: cloudForm.control, name: "display_name" }) ?? "";

	const startOauth = useLibraryMutation("cloud.oauth.start");
	const cancelOauth = useLibraryMutation("cloud.oauth.cancel");

	const pollQuery = useLibraryQuery(
		{
			type: "cloud.oauth.poll",
			input: (flowId ? { flow_id: flowId } : { flow_id: "" }) as PollInput,
		},
		{
			enabled: flowId !== null,
			refetchInterval: flowId !== null ? 1000 : false,
			refetchOnWindowFocus: false,
			retry: false,
		},
	);

	const status: OauthFlowStatus | undefined = (pollQuery.data as PollOutput | undefined)
		?.status;
	const pollingActive = flowId !== null && status?.type !== "completed";

	const canSubmit = useMemo(() => {
		if (flowId !== null) return false;
		if (!isValidClientId(clientId)) return false;
		if (!clientSecret.trim()) return false;
		return true;
	}, [flowId, clientId, clientSecret]);

	const resetFlow = useCallback(() => {
		setFlowId(null);
		setAuthUrl(null);
		setRedirectUri(null);
	}, []);

	const handleConnect = useCallback(async () => {
		setError(null);
		try {
			const input: StartInput = {
				provider: "onedrive",
				client_id: clientId.trim(),
				client_secret: clientSecret,
			};
			const output = (await startOauth.mutateAsync(input)) as StartOutput;
			setFlowId(output.flow_id);
			setAuthUrl(output.auth_url);
			setRedirectUri(output.redirect_uri);
			platform.openLink(output.auth_url);
		} catch (e) {
			setError({
				kind: "start",
				message:
					e instanceof Error
						? e.message
						: "Failed to start Microsoft sign-in.",
			});
		}
	}, [clientId, clientSecret, platform, startOauth]);

	const handleCancel = useCallback(async () => {
		const id = flowId;
		resetFlow();
		if (!id) return;
		try {
			const input: CancelInput = { flow_id: id };
			await cancelOauth.mutateAsync(input);
		} catch (e) {
			// Best-effort cancel: if the flow already completed between the
			// click and the call, the backend returns `cancelled: false` and
			// that's fine. Any other failure is non-actionable here.
			// eslint-disable-next-line no-console
			console.warn("cloud.oauth.cancel failed", e);
		}
	}, [flowId, cancelOauth, resetFlow]);

	// Abandon the loopback listener if the user closes the modal mid-flow
	// so the daemon does not hold the port for the full 5 min TTL.
	useEffect(() => {
		if (flowId === null) return;
		return () => {
			const input: CancelInput = { flow_id: flowId };
			// Fire-and-forget: the component is unmounting so we cannot await.
			void cancelOauth.mutateAsync(input).catch(() => undefined);
		};
	}, [flowId, cancelOauth]);

	// Terminal poll status: completed delegates to the parent's
	// volumes.add_cloud submit to keep that pipeline in one place.
	useEffect(() => {
		if (!status) return;
		if (status.type === "completed") {
			const { access_token, refresh_token } = status.tokens;
			cloudForm.setValue("access_token", access_token);
			cloudForm.setValue("refresh_token", refresh_token ?? "");
			if (!displayName && status.display_name) {
				cloudForm.setValue("display_name", status.display_name);
			}
			// Stop polling before firing the parent submit so the poll
			// query does not refetch a now-useless flow id.
			setFlowId(null);
			setAuthUrl(null);
			setRedirectUri(null);
			void onSubmitCloud();
		} else if (status.type === "failed") {
			setError({ kind: "poll", message: status.error });
			resetFlow();
		} else if (status.type === "cancelled") {
			setError({ kind: "poll", message: "Sign-in cancelled." });
			resetFlow();
		}
	}, [status, cloudForm, onSubmitCloud, displayName, resetFlow]);

	const handleCopyRedirects = useCallback(async () => {
		const payload = buildRedirectUrisClipboardPayload();
		try {
			await navigator.clipboard.writeText(payload);
			setCopiedRedirects(true);
			window.setTimeout(() => setCopiedRedirects(false), 1500);
		} catch {
			setCopiedRedirects(false);
		}
	}, []);

	const handleOpenPortal = useCallback(() => {
		platform.openLink(AZURE_PORTAL_APP_REGISTRATIONS_URL);
	}, [platform]);

	const pollingMessage = statusToMessage(status);

	return (
		<div className="space-y-4">
			<div className="space-y-2">
				<Label>Display Name</Label>
				<Input
					{...cloudForm.register("display_name")}
					size="md"
					placeholder="My OneDrive"
					className="bg-app-input"
					disabled={pollingActive}
				/>
			</div>

			<Tutorial
				open={tutorialOpen}
				onToggle={() => setTutorialOpen((v) => !v)}
				onOpenPortal={handleOpenPortal}
				onCopyRedirects={handleCopyRedirects}
				copied={copiedRedirects}
			/>

			<div className="space-y-2">
				<Label>Application (client) ID</Label>
				<Input
					{...cloudForm.register("client_id")}
					size="md"
					placeholder="00000000-0000-4000-8000-000000000000"
					className="bg-app-input"
					disabled={pollingActive}
					autoComplete="off"
					spellCheck={false}
				/>
				{clientId.trim().length > 0 && !isValidClientId(clientId) && (
					<p className="text-xs text-red-500">
						Expected a UUID v4 like
						00000000-0000-4000-8000-000000000000.
					</p>
				)}
			</div>

			<div className="space-y-2">
				<Label>Client secret</Label>
				<Input
					{...cloudForm.register("client_secret")}
					type="password"
					size="md"
					placeholder="Paste the secret value (shown only once in Azure)"
					className="bg-app-input"
					disabled={pollingActive}
					autoComplete="off"
					spellCheck={false}
				/>
			</div>

			{pollingActive ? (
				<AwaitingBrowserPanel
					message={pollingMessage ?? "Waiting for Microsoft sign-in…"}
					authUrl={authUrl}
					redirectUri={redirectUri}
					onReopen={() => {
						if (authUrl) platform.openLink(authUrl);
					}}
					onCancel={handleCancel}
					cancelling={cancelOauth.isPending}
				/>
			) : (
				<Button
					type="button"
					variant="accent"
					size="md"
					className="w-full"
					disabled={!canSubmit || startOauth.isPending || isAddingVolume}
					onClick={handleConnect}
				>
					{startOauth.isPending
						? "Preparing sign-in…"
						: isAddingVolume
							? "Adding OneDrive…"
							: "Connect with Microsoft"}
				</Button>
			)}

			{error && (
				<div className="rounded-lg border border-red-500/50 bg-red-500/10 p-3 text-xs text-ink">
					<p className="font-medium">Sign-in error</p>
					<p className="mt-1 text-ink-dull">{error.message}</p>
				</div>
			)}
		</div>
	);
}

interface TutorialProps {
	open: boolean;
	onToggle: () => void;
	onOpenPortal: () => void;
	onCopyRedirects: () => void | Promise<void>;
	copied: boolean;
}

function Tutorial({
	open,
	onToggle,
	onOpenPortal,
	onCopyRedirects,
	copied,
}: TutorialProps) {
	return (
		<div className="rounded-lg border border-app-line bg-app-box">
			<button
				type="button"
				onClick={onToggle}
				aria-expanded={open}
				className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm font-medium text-ink hover:bg-app-hover"
			>
				<CaretRight
					className={clsx(
						"size-4 shrink-0 transition-transform",
						open && "rotate-90",
					)}
					weight="bold"
				/>
				<span className="flex-1">
					How to get your Azure AD client credentials
				</span>
				<span className="text-xs font-normal text-ink-faint">
					{open ? "Hide" : "Show"} steps
				</span>
			</button>
			{open && (
				<div className="space-y-4 border-t border-app-line p-4">
					<div className="flex flex-wrap gap-2">
						<Button
							type="button"
							variant="gray"
							size="sm"
							onClick={onOpenPortal}
						>
							<ArrowSquareOut
								className="mr-1.5 size-3.5"
								weight="bold"
							/>
							Open Azure Portal
						</Button>
						<Button
							type="button"
							variant="gray"
							size="sm"
							onClick={() => {
								void onCopyRedirects();
							}}
						>
							<Copy className="mr-1.5 size-3.5" weight="bold" />
							{copied ? "Copied!" : "Copy redirect URIs"}
						</Button>
					</div>
					<p className="text-xs text-ink-dull">
						You only need to do this once per Microsoft account.
						Spacedrive never sees your Azure credentials — they live
						in your library, encrypted.
					</p>
					<ol className="space-y-4">
						{TUTORIAL_STEPS.map((step, index) => (
							<TutorialStepCard
								key={step.title}
								number={index + 1}
								step={step}
							/>
						))}
					</ol>
				</div>
			)}
		</div>
	);
}

function TutorialStepCard({
	number,
	step,
}: {
	number: number;
	step: TutorialStep;
}) {
	return (
		<li className="space-y-2">
			<div className="flex items-start gap-2">
				<span className="mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full bg-accent/20 text-[10px] font-semibold text-accent">
					{number}
				</span>
				<div className="flex-1 space-y-1">
					<p className="text-sm font-medium text-ink">{step.title}</p>
					<p className="text-xs text-ink-dull">{step.body}</p>
				</div>
			</div>
			<div
				role="img"
				aria-label={`Screenshot placeholder for step ${number}`}
				className="flex aspect-video w-full items-center justify-center rounded-lg border border-dashed border-app-line bg-app-box text-xs text-ink-faint"
			>
				[Screenshot: step {number} — {step.caption}]
			</div>
		</li>
	);
}

interface AwaitingBrowserPanelProps {
	message: string;
	authUrl: string | null;
	redirectUri: string | null;
	onReopen: () => void;
	onCancel: () => void | Promise<void>;
	cancelling: boolean;
}

function AwaitingBrowserPanel({
	message,
	authUrl,
	redirectUri,
	onReopen,
	onCancel,
	cancelling,
}: AwaitingBrowserPanelProps) {
	return (
		<div className="space-y-3 rounded-lg border border-accent/40 bg-accent/5 p-4">
			<div className="flex items-start gap-2">
				<span
					className="mt-1 size-2 shrink-0 animate-pulse rounded-full bg-accent"
					aria-hidden="true"
				/>
				<div className="flex-1 space-y-1">
					<p className="text-sm font-medium text-ink">{message}</p>
					<p className="text-xs text-ink-dull">
						A new browser tab should have opened. Sign in with your
						Microsoft account and accept the requested permissions.
					</p>
					{redirectUri && (
						<p className="text-[11px] text-ink-faint">
							Waiting on redirect to{" "}
							<span className="font-mono">{redirectUri}</span>
						</p>
					)}
				</div>
			</div>
			<div className="flex flex-wrap gap-2">
				{authUrl && (
					<Button
						type="button"
						variant="gray"
						size="sm"
						onClick={onReopen}
					>
						<ArrowSquareOut
							className="mr-1.5 size-3.5"
							weight="bold"
						/>
						Reopen browser
					</Button>
				)}
				<Button
					type="button"
					variant="gray"
					size="sm"
					onClick={() => {
						void onCancel();
					}}
					disabled={cancelling}
				>
					<X className="mr-1.5 size-3.5" weight="bold" />
					{cancelling ? "Cancelling…" : "Cancel sign-in"}
				</Button>
			</div>
		</div>
	);
}
