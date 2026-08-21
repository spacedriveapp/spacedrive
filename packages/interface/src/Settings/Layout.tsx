import type { ReactNode } from "react";
import clsx from "clsx";
import { Select, SelectOption, Switch } from "@spacedrive/primitives";

/**
 * Scroll container for a settings page. Owns the page's padding and the
 * rhythm between sections so pages never set their own outer spacing.
 */
export function SettingsPageContent({
	children,
	className,
}: {
	children: ReactNode;
	className?: string;
}) {
	return (
		<div className="h-full overflow-y-auto">
			<div className={clsx("max-w-2xl space-y-7 px-6 py-6", className)}>
				{children}
			</div>
		</div>
	);
}

/**
 * A titled group of settings. The heading sits outside the card so a page
 * reads as labelled groups rather than a stack of undifferentiated boxes.
 */
export function Section({
	title,
	description,
	right,
	children,
}: {
	title: string;
	description?: ReactNode;
	right?: ReactNode;
	children: ReactNode;
}) {
	return (
		<section className="space-y-2.5">
			<header className="flex items-end justify-between gap-3 px-1">
				<div className="flex min-w-0 flex-col gap-0.5">
					<h3 className="text-sm font-semibold tracking-tight text-ink">
						{title}
					</h3>
					{description && (
						<div className="text-xs leading-relaxed text-ink-faint">
							{description}
						</div>
					)}
				</div>
				{right && (
					<div className="flex shrink-0 items-center gap-2">{right}</div>
				)}
			</header>
			{children}
		</section>
	);
}

/** Surface for free-form section content. Pass `padded` for inline content. */
export function SectionCard({
	padded,
	className,
	children,
}: {
	padded?: boolean;
	className?: string;
	children: ReactNode;
}) {
	return (
		<div
			className={clsx(
				"overflow-hidden rounded-xl border border-app-line bg-app-box",
				padded && "px-4 py-4",
				className,
			)}
		>
			{children}
		</div>
	);
}

/** Surface for a list of `SettingItem` rows separated by hairlines. */
export function SectionRows({ children }: { children: ReactNode }) {
	return (
		<SectionCard className="divide-y divide-app-line">{children}</SectionCard>
	);
}

/** A single labelled row with its control aligned to the trailing edge. */
export function SettingItem({
	label,
	description,
	children,
}: {
	label: string;
	description?: ReactNode;
	children: ReactNode;
}) {
	return (
		<div className="flex flex-row items-center justify-between gap-4 px-4 py-3.5">
			<div className="flex min-w-0 flex-1 flex-col gap-0.5">
				<p className="text-sm text-ink">{label}</p>
				{description && (
					<p className="text-tiny leading-snug text-ink-faint">{description}</p>
				)}
			</div>
			<div className="flex shrink-0 items-center">{children}</div>
		</div>
	);
}

export function ToggleSettingItem({
	label,
	description,
	value,
	onChange,
	disabled,
}: {
	label: string;
	description?: ReactNode;
	value: boolean;
	onChange: (value: boolean) => void;
	disabled?: boolean;
}) {
	return (
		<SettingItem label={label} description={description}>
			<Switch
				size="sm"
				checked={value}
				onCheckedChange={onChange}
				disabled={disabled}
			/>
		</SettingItem>
	);
}

export function SelectSettingItem<TValue extends string>({
	label,
	description,
	value,
	onChange,
	options,
	disabled,
}: {
	label: string;
	description?: ReactNode;
	value: TValue;
	onChange: (value: TValue) => void;
	options: { value: TValue; label: string }[];
	disabled?: boolean;
}) {
	return (
		<SettingItem label={label} description={description}>
			<Select
				value={value}
				onChange={onChange}
				disabled={disabled}
				className="min-w-32"
			>
				{options.map((option) => (
					<SelectOption key={option.value} value={option.value}>
						{option.label}
					</SelectOption>
				))}
			</Select>
		</SettingItem>
	);
}

/** Inset block for supporting copy. Sits inside a padded card or below one. */
export function SettingsNote({ children }: { children: ReactNode }) {
	return (
		<div className="rounded-lg bg-app-input px-3 py-2.5 text-tiny leading-relaxed text-ink-faint">
			{children}
		</div>
	);
}

/** Read-only value rendered in the same inset style as `SettingsNote`. */
export function SettingsValue({
	children,
	className,
}: {
	children: ReactNode;
	className?: string;
}) {
	return (
		<div
			className={clsx(
				"min-w-0 rounded-lg border border-app-line bg-app-input px-3 py-2",
				className,
			)}
		>
			<span className="block truncate font-mono text-xs text-ink-dull">
				{children}
			</span>
		</div>
	);
}

export function SettingsEmptyState({
	title,
	description,
}: {
	title: string;
	description: string;
}) {
	return (
		<SectionCard className="px-4 py-10 text-center">
			<p className="text-sm text-ink">{title}</p>
			<p className="mt-1 text-xs text-ink-faint">{description}</p>
		</SectionCard>
	);
}

/** Placeholder shown while a page's config query is in flight. */
export function SettingsSkeleton() {
	return (
		<SettingsPageContent>
			{[3, 2].map((rows, section) => (
				<div key={section} className="space-y-2.5">
					<div className="space-y-1.5 px-1">
						<div className="h-4 w-28 animate-pulse rounded-full bg-app-line" />
						<div className="h-3 w-64 max-w-full animate-pulse rounded-full bg-app-line" />
					</div>
					<SectionCard className="divide-y divide-app-line">
						{Array.from({ length: rows }, (_, row) => (
							<div key={row} className="space-y-2 px-4 py-3.5">
								<div className="h-3.5 w-40 animate-pulse rounded-full bg-app-line" />
								<div className="h-3 w-56 max-w-full animate-pulse rounded-full bg-app-line" />
							</div>
						))}
					</SectionCard>
				</div>
			))}
		</SettingsPageContent>
	);
}
