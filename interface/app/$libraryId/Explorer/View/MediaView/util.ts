import dayjs from 'dayjs';
import { ExplorerItem, getExplorerItemData, OrderingKey } from '@sd/client';

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

		// TODO: Uncomment when we add sorting by date taken
		// case 'object.mediaData.epochTime': {
		// 	firstFilePathDate = firstFilePath.dateTaken;
		// 	lastFilePathDate = lastFilePath.dateTaken;
		// 	break;
		// }
	}
}
