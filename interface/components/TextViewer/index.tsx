import { useVirtualizer, VirtualItem } from '@tanstack/react-virtual';
import clsx from 'clsx';
import Prism from 'prismjs';
import { memo, useEffect, useRef, useState } from 'react';

import * as prism from './prism';

export interface TextViewerProps {
	src: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	codeExtension?: string;
}

// TODO: ANSI support

export const TextViewer = memo(({ src, onLoad, onError, codeExtension }: TextViewerProps) => {
	const [lines, setLines] = useState<string[]>([]);

	const parentRef = useRef<HTMLPreElement>(null);
	const rowVirtualizer = useVirtualizer({
		count: lines.length,
		getScrollElement: () => parentRef.current,
		estimateSize: () => 25
	});

	useEffect(() => {
		// Ignore empty urls
		if (!src || src === '#') return;

		// TODO: Max out at 128MB of loaded data -> Like Apple do

		let fileOffset = 0;
		const controller = new AbortController();
		fetch(src, {
			mode: 'cors',
			signal: controller.signal
		})
			.then(async (response) => {
				if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
				if (!response.body) return;
				onLoad?.(new UIEvent('load', {}));

				const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
				const ingestLines = async () => {
					const { done, value } = await reader.read();
					if (done) return;
					fileOffset += value.length;
					console.log(fileOffset);

					const chunks = value.split('\n');

					setLines((lines) => [...lines, ...chunks]);

					await ingestLines();
				};
				ingestLines();
			})
			.catch((error) => {
				if (!controller.signal.aborted)
					onError?.(new ErrorEvent('error', { message: `${error}` }));
			});

		return () => controller.abort();
	}, [src, onError, onLoad, codeExtension]);

	return (
		<pre
			ref={parentRef}
			tabIndex={0}
			className={clsx('h-full w-full overflow-x-hidden overflow-y-scroll')}
		>
			<div
				tabIndex={0}
				className={clsx(
					'relative w-full whitespace-pre text-ink',
					codeExtension &&
						`language-${prism.languageMapping.get(codeExtension) ?? codeExtension}`
				)}
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`
				}}
			>
				{rowVirtualizer.getVirtualItems().map((row) => (
					<TextRow
						key={row.key}
						codeExtension={codeExtension}
						row={row}
						content={lines[row.index]!}
					/>
				))}
			</div>
		</pre>
	);
});

function TextRow({
	codeExtension,
	row,
	content
}: {
	codeExtension?: string;
	row: VirtualItem;
	content: string;
}) {
	const contentRef = useRef<HTMLSpanElement>(null);

	useEffect(() => {
		const cb: IntersectionObserverCallback = (events) => {
			for (const event of events) {
				if (
					!event.isIntersecting ||
					contentRef.current?.getAttribute('data-highlighted') === 'true'
				)
					continue;
				contentRef.current?.setAttribute('data-highlighted', 'true');
				Prism.highlightElement(event.target, false); // Prism's async seems to be broken
			}
		};

		if (contentRef.current) new IntersectionObserver(cb).observe(contentRef.current);
	});

	return (
		<div
			className={clsx('absolute left-0 top-0 flex w-full whitespace-pre')}
			style={{
				height: `${row.size}px`,
				transform: `translateY(${row.start}px)`
			}}
		>
			{codeExtension && (
				<div
					key={row.key}
					className={clsx(
						'token block w-[3.8em] shrink-0 whitespace-pre pl-1 text-end',
						row.index % 2 && 'bg-black/40'
					)}
				>
					{row.index + 1}
				</div>
			)}
			<span ref={contentRef} className="flex-1 pl-2">
				{content}
			</span>
		</div>
	);
}
