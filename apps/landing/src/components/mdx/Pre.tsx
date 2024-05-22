'use client';

import { Check } from '@phosphor-icons/react';
import { Copy } from '@phosphor-icons/react/dist/ssr';
import { FC, useRef, useState } from 'react';

const Pre: FC<{ children: React.ReactNode }> = ({ children }) => {
	const textInput = useRef<HTMLDivElement | null>(null);
	const [copied, setCopied] = useState(false);

	const onCopy = () => {
		setCopied(true);
		navigator.clipboard.writeText(textInput.current!.textContent ?? '');
		setTimeout(() => {
			setCopied(false);
		}, 2000);
	};

	return (
		<div ref={textInput} className="relative">
			<button
				aria-label="Copy code"
				type="button"
				className="absolute right-2 top-2 z-10 rounded-md bg-app-box p-3 text-white/60 transition-colors duration-200 ease-in-out hover:bg-app-darkBox"
				onClick={onCopy}
			>
				{copied ? (
					<Check className="my-0 size-5 text-green-400/60" />
				) : (
					<Copy className="my-0 size-5 text-white" />
				)}
			</button>
			<pre className="language-container">{children}</pre>
		</div>
	);
};

export default Pre;
