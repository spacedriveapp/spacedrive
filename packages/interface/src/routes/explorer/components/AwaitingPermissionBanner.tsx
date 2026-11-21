import { LockSimple } from "@phosphor-icons/react";
import { useExplorerFiles } from "../hooks/useExplorerFiles";

/**
 * Shown while the open folder waits on permission: its open did not answer,
 * which on macOS is a privacy prompt nobody has answered. The listing fills in
 * once access is granted.
 */
export function AwaitingPermissionBanner() {
	const { awaitingPermission, files } = useExplorerFiles();
	if (!awaitingPermission || files.length > 0) return null;

	return (
		<div className="mx-2 mb-1.5 flex items-center gap-3 rounded-md border border-app-line bg-app-box px-3 py-1.5 text-xs">
			<LockSimple className="size-4 flex-shrink-0 text-ink-dull" weight="fill" />
			<div className="min-w-0 flex-1">
				<span className="font-medium text-ink">Waiting for permission to open this folder.</span>
				<span className="text-ink-dull">
					{" "}
					Allow access in the system prompt or in your privacy settings. The folder fills in once
					access is granted.
				</span>
			</div>
		</div>
	);
}
