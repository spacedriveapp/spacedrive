import dayjs from 'dayjs';

const DATE_FORMAT = 'D MMM YYYY';

export const formatDate = (date: Date | { from: Date; to: Date }) => {
	if (date instanceof Date) return dayjs(date).format(DATE_FORMAT);

	const sameMonth = date.from.getMonth() === date.to.getMonth();
	const sameYear = date.from.getFullYear() === date.to.getFullYear();

	const fromDateFormat = ['D', !sameMonth && 'MMM', !sameYear && 'YYYY']
		.filter(Boolean)
		.join(' ');

	return `${dayjs(date.from).format(fromDateFormat)} - ${dayjs(date.to).format(DATE_FORMAT)}`;
};
