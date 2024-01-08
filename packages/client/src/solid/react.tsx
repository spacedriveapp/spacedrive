import { useEffect, useRef } from 'react';
import { JSX as SolidJSX } from 'solid-js';
import { render } from 'solid-js/web';

type Props = {
	root: () => SolidJSX.Element;
};

export function WithSolid({ root }: Props) {
	const ref = useRef<HTMLDivElement>(null);

	// TODO: Inject all context's
	useEffect(() => {
		let cleanup = () => {};
		if (ref.current) cleanup = render(root, ref.current);
		return cleanup;
	}, [root]);

	return <div ref={ref} />;
}
