import {
	createElement,
	createContext as createReactContext,
	isValidElement,
	JSX as ReactJSX,
	useEffect,
	useContext as useReactContext,
	useRef
} from 'react';
import {
	children,
	createContext as createSolidContext,
	Owner,
	JSX as SolidJSX,
	useContext as useSolidContext
} from 'solid-js';
import { createStore, type Store } from 'solid-js/store';

import { insideReactRender } from './internal';
import { useObserver, useObserverWithOwner } from './useObserver';

type RegisteredContext = {
	id: symbol;
	store: Store<any>;
};

const reactGlobalContext = createReactContext([] as RegisteredContext[]);
const solidGlobalContext = createSolidContext(() => [] as RegisteredContext[]);

export function createSharedContext<T>(initialValue: T) {
	const id = Symbol('shared-context');

	function Provider<C>(props: { value: T; children: C }): C {
		const isSolid =
			'get' in Object.getOwnPropertyDescriptor(props, 'children')! ||
			!isValidElement(props.children);

		const ctxEntry: RegisteredContext = {
			id,
			store: () => props.value
		};

		if (isSolid) {
			const globalCtx = useSolidContext(solidGlobalContext);

			return solidGlobalContext.Provider({
				value: () => [...globalCtx().filter((c) => c.id !== id), ctxEntry],
				get children() {
					return props.children as SolidJSX.Element;
				}
			}) as any;
		} else {
			const globalCtx = useReactContext(reactGlobalContext);

			return createElement(
				reactGlobalContext.Provider as any,
				{
					value: [...globalCtx.filter((c) => c.id !== id), ctxEntry]
				},
				props.children as any
			) as any;
		}
	}

	return {
		Provider,
		useContext: () => {
			const isInsideReact = insideReactRender();
			let globalCtx: any;
			if (isInsideReact) {
				// eslint-disable-next-line react-hooks/rules-of-hooks
				globalCtx = useReactContext(reactGlobalContext);
			} else {
				// eslint-disable-next-line react-hooks/rules-of-hooks
				globalCtx = useSolidContext(solidGlobalContext);
			}

			let reactObserver: T | undefined = undefined;

			return () => {
				const ctx = ((isInsideReact ? globalCtx : globalCtx()) as RegisteredContext[]).find(
					(ctx) => ctx.id === id
				);
				if (!ctx) return initialValue;

				if (isInsideReact) {
					if (!reactObserver) reactObserver = useObserver(() => ctx.store() as T);
					return reactObserver as T; // This function doesn't do anything other than make the API consistent
				} else {
					return ctx.store() as T;
				}
			};
		}
	};
}

export function useWithContextReact(): (elem: () => SolidJSX.Element) => SolidJSX.Element {
	const globalCtx = useReactContext(reactGlobalContext);
	const ref = useRef(createStore<RegisteredContext[]>([]));

	useEffect(() => ref.current[1](globalCtx), [globalCtx, ref]);

	return (elem) =>
		solidGlobalContext.Provider({
			value: () => ref.current[0],
			children: elem as any
		});
}

export function withReactCtx(owner: Owner, elem: ReactJSX.Element) {
	// eslint-disable-next-line react-hooks/rules-of-hooks
	const globalCtx = useObserverWithOwner(owner, () => {
		// eslint-disable-next-line react-hooks/rules-of-hooks
		return useSolidContext(solidGlobalContext)();
	});

	return createElement(reactGlobalContext.Provider, { value: globalCtx }, elem);
}
