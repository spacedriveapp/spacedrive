import { useQuery } from "@tanstack/react-query";
import type { File, LocalPathInput, LocalPathOutput } from "@sd/ts-client";
import { WIRE_METHODS } from "@sd/ts-client";
import { toast } from "@spacedrive/primitives";
import { usePlatform, type OpenResult } from "../contexts/PlatformContext";
import { useSpacedriveClient } from "../contexts/SpacedriveContext";

/**
 * Open files in other apps.
 *
 * A file on this device opens at its own path. A file on another device opens
 * at its path inside the share the daemon mounts on this machine, which
 * streams what the app reads; its own path would name a different file here,
 * or none. The app list comes only from files on this device, since listing
 * apps for another device's file would mount the share for every row drawn.
 */
export function useOpenWith(files: File[]) {
	const platform = usePlatform();
	const client = useSpacedriveClient();

	const localPaths = files.flatMap((file) =>
		file.kind === "File" && file.is_local && "Physical" in file.sd_path
			? [file.sd_path.Physical.path]
			: [],
	);

	const { data: apps, isLoading } = useQuery({
		queryKey: ["openWith", ...localPaths],
		queryFn: async () => {
			if (!platform.getAppsForPaths) {
				return [];
			}
			return platform.getAppsForPaths(localPaths);
		},
		enabled:
			localPaths.length > 0 &&
			localPaths.length === files.length &&
			!!platform.getAppsForPaths,
	});

	const pathOnThisMachine = async (file: File): Promise<string> => {
		if (!("Physical" in file.sd_path)) {
			throw new Error(`${file.name} has no path on this machine`);
		}
		if (file.is_local) {
			return file.sd_path.Physical.path;
		}
		const output = await client.execute<LocalPathInput, LocalPathOutput>(
			WIRE_METHODS.coreQueries["files.local_path"],
			{ path: file.sd_path },
		);
		return output.path;
	};

	const openWithDefault = async (file: File) => {
		if (!platform.openPathDefault) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
			const result = await platform.openPathDefault(await pathOnThisMachine(file));
			handleOpenResult(result);
		} catch (e) {
			toast.error(`Failed to open file: ${e}`);
		}
	};

	const openWithApp = async (file: File, appId: string) => {
		if (!platform.openPathWithApp) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
			const result = await platform.openPathWithApp(await pathOnThisMachine(file), appId);
			handleOpenResult(result);
		} catch (e) {
			toast.error(`Failed to open file: ${e}`);
		}
	};

	const openMultipleWithApp = async (files: File[], appId: string) => {
		if (!platform.openPathsWithApp) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
			const paths = await Promise.all(files.map(pathOnThisMachine));
			const results = await platform.openPathsWithApp(paths, appId);
			results.forEach(handleOpenResult);
		} catch (e) {
			toast.error(`Failed to open files: ${e}`);
		}
	};

	return {
		apps: apps ?? [],
		isLoading,
		openWithDefault,
		openWithApp,
		openMultipleWithApp,
	};
}

function handleOpenResult(result: OpenResult) {
	switch (result.status) {
		case "success":
			// Silent success
			break;
		case "file_not_found":
			toast.error(`File not found: ${result.path}`);
			break;
		case "app_not_found":
			toast.error(`Application not found: ${result.app_id}`);
			break;
		case "permission_denied":
			toast.error(`Permission denied: ${result.path}`);
			break;
		case "platform_error":
			toast.error(`Error: ${result.message}`);
			break;
	}
}
