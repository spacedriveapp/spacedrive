import { useExplorer } from "../../context";
import { GridView } from "../GridView";

export function SearchView() {
	const explorer = useExplorer();

	// If not in search mode, don't render
	if (explorer.mode.type !== "search") {
		return null;
	}

	const { query, scope } = explorer.mode;

	// TODO: Implement actual search query
	// For now, just render a placeholder message
	return (
		<div className="flex flex-col items-center justify-center h-full p-8 text-center">
			<h2 className="text-2xl font-bold mb-4">Search</h2>
			<p className="text-ink-dull mb-4">
				Searching for: <span className="font-mono">{query}</span>
			</p>
			<p className="text-ink-faint text-sm">
				Search implementation in progress...
			</p>
		</div>
	);
}
