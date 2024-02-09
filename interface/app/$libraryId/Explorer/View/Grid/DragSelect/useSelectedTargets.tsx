import { RefObject, useCallback, useRef } from 'react';
import Selecto from 'react-selecto';

export const useSelectedTargets = (selecto: RefObject<Selecto>) => {
	const selectedTargets = useRef(new Map<string, HTMLElement>());

	const addSelectedTarget = useCallback(
		(id: string, node: HTMLElement, options = { updateSelecto: true }) => {
			selectedTargets.current.set(id, node);
			if (!options.updateSelecto) return;
			selecto.current?.setSelectedTargets([...selectedTargets.current.values()]);
		},
		[selecto]
	);

	const removeSelectedTarget = useCallback(
		(id: string, options = { updateSelecto: true }) => {
			selectedTargets.current.delete(id);
			if (!options.updateSelecto) return;
			selecto.current?.setSelectedTargets([...selectedTargets.current.values()]);
		},
		[selecto]
	);

	const resetSelectedTargets = useCallback(
		(targets: { id: string; node: HTMLElement }[] = [], options = { updateSelecto: true }) => {
			selectedTargets.current = new Map(targets.map(({ id, node }) => [id, node]));
			if (!options.updateSelecto) return;
			selecto.current?.setSelectedTargets([...selectedTargets.current.values()]);
		},
		[selecto]
	);

	return { selectedTargets, addSelectedTarget, removeSelectedTarget, resetSelectedTargets };
};
