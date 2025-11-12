import { Spacedrop } from '@sd/interface';

const samplePeople = [
	{ id: '1', name: 'Jamie', initials: 'JP', status: 'online' as const },
	{ id: '2', name: 'Alex', initials: 'AB', status: 'online' as const },
	{ id: '3', name: 'Sam', initials: 'SC', status: 'offline' as const },
	{ id: '4', name: 'Morgan', initials: 'MJ', status: 'online' as const },
	{ id: '5', name: 'Taylor', initials: 'TW', status: 'online' as const },
	{ id: '6', name: 'Jordan', initials: 'JK', status: 'offline' as const },
	{ id: '7', name: 'Casey', initials: 'CD', status: 'online' as const },
	{ id: '8', name: 'Riley', initials: 'RM', status: 'online' as const }
];

export function SpacedropWindow() {
	return (
		<div className="h-screen w-screen bg-sidebar">
			<Spacedrop people={samplePeople} onClose={() => window.close()} />
		</div>
	);
}
