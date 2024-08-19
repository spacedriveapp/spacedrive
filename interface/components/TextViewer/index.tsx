import { useVirtualizer, VirtualItem } from '@tanstack/react-virtual';
import clsx from 'clsx';
import { memo, useEffect, useRef, useState } from 'react';

import { languageMapping } from './prism';

const prismaLazy = import('./prism-lazy');
prismaLazy.catch((e) => console.error('Failed to load prism-lazy', e));

export interface TextViewerProps {
	src: string;
	className?: string;
	onLoad?: (event: HTMLElementEventMap['load']) => void;
	onError?: (event: HTMLElementEventMap['error']) => void;
	codeExtension?: string;
	isSidebarPreview?: boolean;
}

// TODO: ANSI support

export const TextViewer = memo(
	({ src, className, onLoad, onError, codeExtension, isSidebarPreview }: TextViewerProps) => {
		const [lines, setLines] = useState<string[]>([]);
		const parentRef = useRef<HTMLPreElement>(null);
		const rowVirtualizer = useVirtualizer({
			count: lines.length,
			getScrollElement: () => parentRef.current,
			estimateSize: () => 22
		});

		useEffect(() => {
			// Ignore empty urls
			if (!src || src === '#') return;

			const controller = new AbortController();
			fetch(src, {
				mode: 'cors',
				signal: controller.signal
			})
				.then((response) => {
					if (!response.ok) throw new Error(`Invalid response: ${response.statusText}`);
					if (!response.body) return;
					onLoad?.(new UIEvent('load', {}));

					const reader = response.body.pipeThrough(new TextDecoderStream()).getReader();
					return reader.read().then(function ingestLines({
						done,
						value
					}): void | Promise<void> {
						if (done) return;

						const chunks = value.split('\n');
						setLines([...chunks]);

						if (isSidebarPreview) return;

						// Read some more, and call this function again
						return reader.read().then(ingestLines);
					});
				})
				.catch((error) => {
					if (!controller.signal.aborted)
						onError?.(new ErrorEvent('error', { message: `${error}` }));
				});

			return () => controller.abort();
		}, [src, onError, onLoad, codeExtension, isSidebarPreview]);

		return (
			<pre ref={parentRef} tabIndex={0} className={className}>
				<div
					tabIndex={0}
					className={clsx(
						'relative w-full whitespace-pre text-sm text-ink',
						codeExtension &&
							`language-${languageMapping.get(codeExtension) ?? codeExtension}`
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
	}
);

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
		const ref = contentRef.current;
		if (ref == null) return;

		let intersectionObserver: null | IntersectionObserver = null;

		prismaLazy.then(({ highlightElement }) => {
			intersectionObserver = new IntersectionObserver((events) => {
				for (const event of events) {
					if (!event.isIntersecting || ref.getAttribute('data-highlighted') === 'true')
						continue;

					ref.setAttribute('data-highlighted', 'true');
					highlightElement(event.target, false); // Prism's async seems to be broken

					// With this class present TOML headers are broken Eg. `[dependencies]` will format over multiple lines
					const children = ref.children;
					if (children) {
						for (const elem of children) {
							elem.classList.remove('table');
						}
					}
				}
			});
			intersectionObserver.observe(ref);
		});

		return () => intersectionObserver?.disconnect();
	}, []);

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
						'token block shrink-0 whitespace-pre pl-2 pr-4 text-sm leading-6 text-gray-450'
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
