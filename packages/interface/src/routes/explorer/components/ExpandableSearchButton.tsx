import {
	useState,
	useRef,
	useEffect,
	forwardRef,
	useImperativeHandle,
	type KeyboardEvent,
} from "react";
import { motion, AnimatePresence } from "framer-motion";
import { MagnifyingGlass } from "@phosphor-icons/react";
import { CircleButton, SearchBar } from "@spacedrive/primitives";

interface ExpandableSearchFieldProps {
	expanded: boolean;
	onExpand: () => void;
	value: string;
	onChange: (value: string) => void;
	onClear?: () => void;
	onBlur?: () => void;
	onKeyDown?: (event: KeyboardEvent<HTMLInputElement>) => void;
	placeholder?: string;
}

export interface ExpandableSearchFieldHandle {
	focus: () => void;
}

/**
 * A search button that widens into a field. Whoever renders it decides when
 * it is expanded. A collapsed field has no input yet, so focusing one that
 * is still expanding is left to the animation's completion handler.
 */
export const ExpandableSearchField = forwardRef<
	ExpandableSearchFieldHandle,
	ExpandableSearchFieldProps
>(function ExpandableSearchField(
	{
		expanded,
		onExpand,
		value,
		onChange,
		onClear,
		onBlur,
		onKeyDown,
		placeholder = "Search...",
	},
	ref,
) {
	const inputRef = useRef<HTMLInputElement>(null);

	useImperativeHandle(
		ref,
		() => ({
			focus: () => inputRef.current?.focus(),
		}),
		[],
	);

	// Focus input after animation completes
	const handleAnimationComplete = () => {
		if (expanded && inputRef.current) {
			inputRef.current.focus();
		}
	};

	return (
		<motion.div
			animate={{
				width: expanded ? 256 : 32, // w-64 = 256px, button = 32px
			}}
			transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
			className="overflow-hidden"
			onAnimationComplete={handleAnimationComplete}
		>
			<AnimatePresence mode="wait" initial={false}>
				{!expanded ? (
					<motion.div
						key="button"
						initial={{ opacity: 0 }}
						animate={{ opacity: 1 }}
						exit={{ opacity: 0 }}
						transition={{ duration: 0.15 }}
					>
						<CircleButton icon={MagnifyingGlass} onClick={onExpand} />
					</motion.div>
				) : (
					<motion.div
						key="searchbar"
						initial={{ opacity: 0 }}
						animate={{ opacity: 1 }}
						exit={{ opacity: 0 }}
						transition={{ duration: 0.15 }}
					>
						<SearchBar
							ref={inputRef}
							value={value}
							onChange={onChange}
							onClear={onClear}
							placeholder={placeholder}
							className="w-64"
							onBlur={onBlur}
							onKeyDown={onKeyDown}
							autoFocus
						/>
					</motion.div>
				)}
			</AnimatePresence>
		</motion.div>
	);
});

interface ExpandableSearchButtonProps {
	value: string;
	onChange: (value: string) => void;
	onClear: () => void;
	placeholder?: string;
}

/**
 * A field for filtering a page's own list: it expands on click and collapses
 * when it loses focus, or a click lands outside it, while empty.
 */
export function ExpandableSearchButton({
	value,
	onChange,
	onClear,
	placeholder,
}: ExpandableSearchButtonProps) {
	const [isExpanded, setIsExpanded] = useState(false);
	const containerRef = useRef<HTMLDivElement>(null);

	// Expand if there's a value
	useEffect(() => {
		if (value) {
			setIsExpanded(true);
		}
	}, [value]);

	// Collapse when clicking outside
	useEffect(() => {
		const handleClickOutside = (event: MouseEvent) => {
			if (
				containerRef.current &&
				!containerRef.current.contains(event.target as Node) &&
				isExpanded &&
				!value
			) {
				setIsExpanded(false);
			}
		};

		if (isExpanded) {
			document.addEventListener("mousedown", handleClickOutside);
			return () => {
				document.removeEventListener("mousedown", handleClickOutside);
			};
		}
	}, [isExpanded, value]);

	// Handle input blur - collapse if empty
	const handleBlur = () => {
		if (!value) {
			setIsExpanded(false);
		}
	};

	return (
		<div ref={containerRef}>
			<ExpandableSearchField
				expanded={isExpanded}
				onExpand={() => setIsExpanded(true)}
				value={value}
				onChange={onChange}
				onClear={onClear}
				onBlur={handleBlur}
				placeholder={placeholder}
			/>
		</div>
	);
}
