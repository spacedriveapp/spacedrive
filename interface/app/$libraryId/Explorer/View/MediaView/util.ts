import dayjs from 'dayjs';
import { ExplorerItem, getExplorerItemData, OrderingKey } from '@sd/client';

export const formatDate = (date: Date | { from: Date; to: Date }, dateFormat: string) => {
	if (date instanceof Date) return dayjs(date).format(dateFormat);

	const sameMonth = date.from.getMonth() === date.to.getMonth();
	const sameYear = date.from.getFullYear() === date.to.getFullYear();

	const fromDateFormat = ['D', !sameMonth && 'MMM', !sameYear && 'YYYY']
		.filter(Boolean)
		.join(' ');

	return `${dayjs(date.from).format(fromDateFormat)} - ${dayjs(date.to).format(dateFormat)}`;
};

export function getDate(item: ExplorerItem, orderBy: OrderingKey) {
	const filePath = getExplorerItemData(item);

	switch (orderBy) {
		case 'dateCreated': {
			return filePath.dateCreated;
		}

		case 'dateIndexed': {
			return filePath.dateIndexed;
		}

		case 'dateModified': {
			return filePath.dateModified;
		}

		case 'object.dateAccessed': {
			return filePath.dateAccessed;
		}

		case 'object.mediaData.epochTime': {
			return filePath.dateTaken;
		}
	}
}
