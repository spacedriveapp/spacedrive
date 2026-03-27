import { useState } from "react";

export function usePopover() {
	const [open, setOpen] = useState(false);

	return { open, setOpen };
}
