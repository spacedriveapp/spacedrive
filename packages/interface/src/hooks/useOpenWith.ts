import { useQuery } from "@tanstack/react-query";
import { usePlatform, type OpenResult } from "../platform";
import { toast } from "@sd/ui";

export function useOpenWith(paths: string[]) {
	const platform = usePlatform();

	const { data: apps, isLoading } = useQuery({
		queryKey: ["openWith", ...paths],
		queryFn: async () => {
			if (!platform.getAppsForPaths) {
				return [];
			}
			return platform.getAppsForPaths(paths);
		},
		enabled: paths.length > 0 && !!platform.getAppsForPaths,
	});

	const openWithDefault = async (path: string) => {
		if (!platform.openPathDefault) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
			const result = await platform.openPathDefault(path);
			handleOpenResult(result);
		} catch (e) {
			toast.error(`Failed to open file: ${e}`);
		}
	};

	const openWithApp = async (path: string, appId: string) => {
		if (!platform.openPathWithApp) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
			const result = await platform.openPathWithApp(path, appId);
			handleOpenResult(result);
		} catch (e) {
			toast.error(`Failed to open file: ${e}`);
		}
	};

	const openMultipleWithApp = async (paths: string[], appId: string) => {
		if (!platform.openPathsWithApp) {
			toast.error("Opening files is not supported on this platform");
			return;
		}

		try {
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
