import type { File } from "@sd/ts-client";

export function formatDate(
	date: Date | { from: Date; to: Date },
	locale = "en-US",
): string {
	if (date instanceof Date) {
		return date.toLocaleDateString(locale, {
			year: "numeric",
			month: "long",
			day: "numeric",
		});
	}

	const sameMonth = date.from.getMonth() === date.to.getMonth();
	const sameYear = date.from.getFullYear() === date.to.getFullYear();

	const fromOptions: Intl.DateTimeFormatOptions = {
		day: "numeric",
		...(sameMonth ? {} : { month: "short" }),
		...(sameYear ? {} : { year: "numeric" }),
	};

	const toOptions: Intl.DateTimeFormatOptions = {
		day: "numeric",
		month: "long",
		year: "numeric",
	};

	const fromStr = date.from.toLocaleDateString(locale, fromOptions);
	const toStr = date.to.toLocaleDateString(locale, toOptions);

	return `${fromStr} - ${toStr}`;
}

export function getItemDate(file: File): string | null {
	// Use modified_at (date modified). Later this will support sort options like date_taken
	return file.modified_at || null;
}

export function normalizeDateToMidnight(dateString: string): Date {
	const date = new Date(dateString);
	date.setHours(0, 0, 0, 0);
	return date;
}