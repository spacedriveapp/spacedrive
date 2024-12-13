'use client';

import { Check } from '@phosphor-icons/react';
import { Copy } from '@phosphor-icons/react/dist/ssr';
import { FC, useRef, useState } from 'react';
import { Button, Tooltip } from '@sd/ui';

const Pre: FC<{ children: React.ReactNode }> = ({ children }) => {
	const textInput = useRef<HTMLDivElement | null>(null);
	const [copied, setCopied] = useState(false);

	const onCopy = () => {
		setCopied(true);
		navigator.clipboard.writeText(textInput.current!.textContent ?? '');
		setTimeout(() => {
			setCopied(false);
		}, 3000);
	};

	return (
		<div ref={textInput} className="relative">
			<Button
				size="md"
				rounding="both"
				variant="outline"
				className="absolute right-2 top-2 z-10 bg-app-box !py-2.5 transition-colors duration-200 ease-in-out hover:bg-app-darkBox"
				onClick={onCopy}
			>
				<Tooltip label={copied ? 'Copied' : 'Copy to clipboard'}>
					{copied ? (
						<Check size={18} className="text-green-400" />
					) : (
						<Copy size={18} className="text-white opacity-70" />
					)}
				</Tooltip>
			</Button>
			<pre className="language-container">{children}</pre>
		</div>
	);
};

export default Pre;
