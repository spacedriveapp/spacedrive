import { useEffect, useRef } from 'react';
import { JSX as SolidJSX } from 'solid-js';
import { render } from 'solid-js/web';

import { useWithContextReact } from './context';

type Props<T> =
	| ({
			root: (props: T) => SolidJSX.Element;
	  } & T)
	| {
			root: () => SolidJSX.Element;
	  };

export function WithSolid<T>(props: Props<T>) {
	const ref = useRef<HTMLDivElement>(null);
	const applyCtx = useWithContextReact();

	useEffect(() => {
		let cleanup = () => {};
		if (ref.current)
			cleanup = render(() => {
				const { root, ...childProps } = props;
				return applyCtx(() => root(childProps as any));
			}, ref.current);
		return cleanup;
	}, [props, applyCtx]);

	return <div ref={ref} />;
}
