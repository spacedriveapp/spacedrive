import {
	createElement,
	createContext as createReactContext,
	isValidElement,
	PropsWithChildren,
	Context as ReactContext,
	JSX as ReactJSX,
	useContext as useReactContext,
	useState
} from 'react';
import {
	createContext as createSolidContext,
	getOwner,
	Owner,
	Context as SolidContext,
	JSX as SolidJSX,
	useContext as useSolidContext
} from 'solid-js';

import { useObserverWithOwner } from './useObserver';

type RegisteredContext = {
	id: symbol;
	reactContext: ReactContext<any>;
	solidContext: SolidContext<any>;
};

const reactGlobalContext = createReactContext([] as RegisteredContext[]);
const solidGlobalContext = createSolidContext([] as RegisteredContext[]);

// TODO: Use context for props to avoid complete rerenders

export function createSharedContext<T>(initialValue: T) {
	const solidContext = createSolidContext(initialValue);
	const reactContext = createReactContext(initialValue);

	const ctxEntry: RegisteredContext = {
		id: solidContext.id,
		reactContext,
		solidContext
	};

	function Provider<C>(props: { value: T; children: C }): C {
		const isSolid =
			'get' in Object.getOwnPropertyDescriptor(props, 'children')! ||
			!isValidElement(props.children);

		if (isSolid) {
			const globalCtx = useSolidContext(solidGlobalContext);
			return solidGlobalContext.Provider({
				value: [...globalCtx, ctxEntry], // TODO: Ensure multiple of the same provider override correctly
				get children() {
					return solidContext.Provider({
						value: props.value,
						get children() {
							return props.children as SolidJSX.Element;
						}
					});
				}
			}) as any;
		} else {
			const globalCtx = useReactContext(reactGlobalContext);
			return createElement(
				reactGlobalContext.Provider as any,
				{ value: [...globalCtx, ctxEntry] }, // TODO: Ensure multiple of the same provider override correctly
				createElement(
					reactContext.Provider as any,
					{
						value: props.value
					},
					props.children as any
				)
			) as any;
		}
	}

	return {
		Provider,
		useContext: () => {
			const isInsideReact = insideReactRender();
			let ctx;
			if (isInsideReact) {
				// eslint-disable-next-line react-hooks/rules-of-hooks
				ctx = useReactContext(reactContext);
			} else {
				// eslint-disable-next-line react-hooks/rules-of-hooks
				ctx = useSolidContext(solidContext);
			}
			// if (!ctx) throw new Error("TODO"); // TODO: Get context name for error
			return ctx as T;
		}
	};
}

function insideReactRender() {
	try {
		// eslint-disable-next-line react-hooks/rules-of-hooks
		useState();
		return true;
	} catch (err) {
		return false;
	}
}

export function useWithContextReact(): (elem: SolidJSX.Element) => SolidJSX.Element {
	const globalCtx = useReactContext(reactGlobalContext);

	return (elem) => {
		// TODO

		return elem;
	};
}

// TODO: Get rid of this
export function useWithContextSolid(): (elem: ReactJSX.Element) => ReactJSX.Element {
	const owner = getOwner()!;
	return (elem) => createElement(WithContext, { owner }, elem);
}

function WithContext(props: PropsWithChildren<{ owner: Owner }>) {
	const contexts = useObserverWithOwner(props.owner, () => {
		const globalCtx = useSolidContext(solidGlobalContext);
		return globalCtx.map((ctx) => [ctx, useSolidContext(ctx.solidContext)] as const);
	});

	let children = props.children;
	contexts?.map(([ctx, value]) => {
		children = createElement(ctx.reactContext.Provider, { value }, children);
	});
	return children;
}
