"use client";

/**
 * Spacedrive-specific Popover wrapper that preserves the usePopover + trigger prop API.
 * Built on top of @spaceui/primitives Radix Popover.
 *
 * Usage:
 *   const popover = usePopover();
 *   <WrappedPopover popover={popover} trigger={<button>Open</button>}>
 *     <div>Content</div>
 *   </WrappedPopover>
 */

import { Popover } from "@spaceui/primitives";
import clsx from "clsx";
import React, { useEffect, useRef } from "react";

import { usePopover } from "../hooks/usePopover";

interface WrappedPopoverProps {
	popover: ReturnType<typeof usePopover>;
	trigger: React.ReactNode;
	disabled?: boolean;
	children?: React.ReactNode;
	className?: string;
	side?: "top" | "right" | "bottom" | "left";
	align?: "start" | "center" | "end";
	sideOffset?: number;
	alignOffset?: number;
}

export function WrappedPopover({
	popover,
	trigger,
	disabled,
	children,
	className,
	...props
}: WrappedPopoverProps) {
	const triggerRef = useRef<HTMLButtonElement>(null);

	useEffect(() => {
		const onResize = () => {
			if (triggerRef.current && triggerRef.current.offsetWidth === 0)
				popover.setOpen(false);
		};

		window.addEventListener("resize", onResize);
		return () => window.removeEventListener("resize", onResize);
	}, [popover.setOpen]);

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger ref={triggerRef} disabled={disabled} asChild>
				{trigger}
			</Popover.Trigger>
			<Popover.Content className={className} {...props}>
				{children}
			</Popover.Content>
		</Popover.Root>
	);
}

export { usePopover };
