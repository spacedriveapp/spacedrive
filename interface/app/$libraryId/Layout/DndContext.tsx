import * as Dnd from '@dnd-kit/core';
import { PropsWithChildren } from 'react';

export const DndContext = ({ children }: PropsWithChildren) => {
	const sensors = Dnd.useSensors(
		Dnd.useSensor(Dnd.PointerSensor, {
			activationConstraint: {
				distance: 4
			}
		})
	);

	return (
		<Dnd.DndContext
			sensors={sensors}
			collisionDetection={Dnd.pointerWithin}
			// We handle scrolling ourselves as dnd-kit
			// auto-scroll is causing issues
			autoScroll={{ enabled: false }}
		>
			{children}
		</Dnd.DndContext>
	);
};
