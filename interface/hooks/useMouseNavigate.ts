import { useNavigate } from "react-router";
import { useSearchStore } from "~/hooks";
import { MouseEvent } from "react";


export const useMouseNavigate = () => {
	const idx = history.state.idx as number;
	const navigate = useNavigate();
	const {isFocused} = useSearchStore();

	const handler = (e: MouseEvent) => {
			if (e.buttons === 8) {
					if (idx === 0 || isFocused) return;
					navigate(-1);
				} else if (e.buttons === 16) {
					if (idx === history.length - 1 || isFocused) return;
					navigate(1);
				}
			}

	return handler;
}
