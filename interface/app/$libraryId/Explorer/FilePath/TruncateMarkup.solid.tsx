/** @jsxImportSource solid-js */

import { createMemo, createSignal, JSX, onCleanup, onMount, Show } from 'solid-js';

const LINE_HEIGHT = 19;

// TODO: Call `props.onTruncate` when the text is truncated
// TODO: This is broken on small names

export function TruncateMarkupSolid(props: {
	lines: number;
	prefix?: JSX.Element;
	postfix?: JSX.Element;
	content: string;
	onTruncate: (wasTruncated: boolean) => void;
	style?: JSX.CSSProperties;
}) {
	const [cutoff, setCutoff] = createSignal(0);
	const cutoffContent = createMemo(() =>
		props.content.slice(0, props.content.length - (cutoff() ?? 0))
	);

	let ref!: HTMLDivElement;

	function truncate() {
		const height = ref.getBoundingClientRect().height;

		if (height / LINE_HEIGHT <= props.lines) return;

		setCutoff((c) => (c ?? 0) + 1);

		if (ref.getBoundingClientRect().height / LINE_HEIGHT <= props.lines) return;

		truncate();
	}

	onMount(() => {
		const observer = new ResizeObserver(() => {
			setCutoff(0);
			truncate();
		});
		observer.observe(ref);
		onCleanup(() => observer.disconnect());
	});

	return (
		<div
			style={{
				'word-break': 'break-word',
				...props.style
			}}
			ref={ref}
		>
			<Show when={props.prefix}>
				<div style={{ display: 'inline-block' }}>{props.prefix}</div>
			</Show>
			{cutoffContent()}
			{props.postfix}
		</div>
	);
}
