import {
	createElement,
	createContext as createReactContext,
	isValidElement,
	useContext as useReactContext,
	useState
} from 'react';
import {
	createContext as createSolidContext,
	JSX as SolidJSX,
	useContext as useSolidContext
} from 'solid-js';

const reactContext = createReactContext(null as string | null);
const solidContext = createSolidContext(null as string | null);

export function createSharedContext<T>(initialValue: T) {
	function Provider<C>(props: { value: T; children: C }): C {
		const isSolid =
			'get' in Object.getOwnPropertyDescriptor(props, 'children')! ||
			!isValidElement(props.children);

		if (isSolid) {
			return solidContext.Provider({
				value: 'solidjs',
				get children() {
					return props.children as SolidJSX.Element;
				}
			}) as any;
		} else {
			return createElement(
				reactContext.Provider,
				{ value: 'react' },
				props.children as any
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
			// if (!ctx) throw new Error("TODO"); // TODO: Get context name
			return { isInsideReact, ctx };
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
