export function shouldNavigate(e: React.MouseEvent): boolean {
	if (e.button !== 0) return false;
	if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return false;
	return true;
}
