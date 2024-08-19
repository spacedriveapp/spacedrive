'use client';

import { Check, Copy } from '@phosphor-icons/react';
import * as Dialog from '@radix-ui/react-dialog';
import { useState } from 'react';
import { Button, Tooltip } from '@sd/ui';

const DOCKER_URL = 'docker pull ghcr.io/spacedriveapp/spacedrive/server';

export function DockerDialog({
	open,
	setOpen
}: {
	open: boolean;
	setOpen: (open: boolean) => void;
}) {
	const [copied, setCopied] = useState(false);

	return (
		<Dialog.Root open={open} onOpenChange={setOpen}>
			<Dialog.Portal>
				<Dialog.Overlay className="fixed inset-0 z-50 bg-app/80 backdrop-blur-sm radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0" />
				<Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[95%] max-w-[500px] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-md border border-app-line bg-app px-2 shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]">
					<div className="p-3 pt-0">
						<h2 className="py-2 text-center text-lg font-semibold text-ink">Docker</h2>
						{/* Link */}
						<div className="flex flex-row items-center">
							<code className="block w-full rounded-md bg-app-darkBox px-3 py-2 text-sm font-medium text-ink">
								$ {DOCKER_URL}
							</code>
							<Button
								size="icon"
								variant="outline"
								rounding="both"
								className="absolute right-6 bg-app-box transition-colors duration-200 ease-in-out hover:bg-app-darkBox"
								onClick={() => {
									navigator.clipboard.writeText(DOCKER_URL);
									setCopied(true);
									setTimeout(() => setCopied(false), 3000);
								}}
							>
								<Tooltip label={copied ? 'Copied' : 'Copy to clipboard'}>
									{copied ? (
										<Check size={18} className="text-green-400" />
									) : (
										<Copy size={18} className="text-white opacity-70" />
									)}
								</Tooltip>
							</Button>
						</div>
						{/* OK Button */}
						<Button
							onClick={() => setOpen(false)}
							variant="accent"
							className="mt-3 w-full !rounded"
							size="md"
						>
							OK
						</Button>
					</div>
				</Dialog.Content>
			</Dialog.Portal>
		</Dialog.Root>
	);
}
