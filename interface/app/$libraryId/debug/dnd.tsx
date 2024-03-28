import { useEffect, useRef } from 'react';
import { usePlatform } from '~/util/Platform';

export function DragAndDropDebug() {
	const ref = useRef<HTMLDivElement>(null);

	const platform = usePlatform();
	useEffect(() => {
		if (!platform.subscribeToDragAndDropEvents) return;

		let finished = false;
		const unsub = platform.subscribeToDragAndDropEvents((event) => {
			if (finished) return;

			console.log(JSON.stringify(event));
			if (!ref.current) return;

			if (event.type === 'Hovered') {
				ref.current.classList.remove('hidden');
				ref.current.style.left = `${event.x}px`;
				ref.current.style.top = `${event.y}px`;
			} else if (event.type === 'Dropped' || event.type === 'Cancelled') {
				ref.current.classList.add('hidden');
			}
		});

		return () => {
			finished = true;
			void unsub.then((unsub) => unsub());
		};
	}, [platform, ref]);

	return <div ref={ref} className="absolute z-[500] hidden size-10 bg-red-500"></div>;
}
