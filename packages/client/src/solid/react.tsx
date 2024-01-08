import { useEffect, useRef } from 'react';
import { JSX as SolidJSX } from 'solid-js';
import { render } from 'solid-js/web';

type Props<T> =
	| ({
			root: (props: T) => SolidJSX.Element;
	  } & T)
	| {
			root: () => SolidJSX.Element;
	  };

export function WithSolid<T>(props: Props<T>) {
	const ref = useRef<HTMLDivElement>(null);

	// TODO: Inject all context's
	useEffect(() => {
		let cleanup = () => {};
		if (ref.current)
			cleanup = render(() => {
				const { root, ...childProps } = props;
				return root(childProps as any);
			}, ref.current);
		return cleanup;
	}, [props]);

	return <div ref={ref} />;
}
