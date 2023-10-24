import { useEffect, useState } from 'react';

export const useScrolled = (ref: React.RefObject<HTMLDivElement>, y = 1) => {
	const [isScrolled, setIsScrolled] = useState(false);

	useEffect(() => {
		const onScroll = () => {
			if (ref.current) {
				if (ref.current.scrollTop >= y) setIsScrolled(true);
				else setIsScrolled(false);
			}
		};

		onScroll();
		ref.current?.addEventListener('scroll', onScroll);
		() => ref.current?.removeEventListener('scroll', onScroll);
	}, [ref, y]);

	return { isScrolled };
};
