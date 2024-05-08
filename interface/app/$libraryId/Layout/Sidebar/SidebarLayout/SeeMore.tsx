import { Children, PropsWithChildren, useState } from 'react';
import { useLocale } from '~/hooks';

export const SEE_MORE_COUNT = 5;

interface Props extends PropsWithChildren {
	limit?: number;
}

export function SeeMore({ children, limit = SEE_MORE_COUNT }: Props) {
	const [seeMore, setSeeMore] = useState(false);

	const childrenArray = Children.toArray(children);

	const { t } = useLocale();
	return (
		<>
			{childrenArray.map((child, index) => (seeMore || index < limit ? child : null))}
			{childrenArray.length > limit && (
				<div
					onClick={() => setSeeMore(!seeMore)}
					className="mb-1 ml-2 mt-0.5 cursor-pointer text-center text-tiny font-semibold text-ink-faint/50 transition hover:text-accent"
				>
					{seeMore ? `${t('see_less')}` : `${t('see_more')}`}
				</div>
			)}
		</>
	);
}
