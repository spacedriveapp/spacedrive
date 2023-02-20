import { useQuery } from '@tanstack/react-query';
import { ScreenHeading } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

export default function DependenciesScreen() {
	const frontEnd = useQuery(['frontend-deps'], () => import('@sd/assets/deps/frontend-deps.json'));
	const backEnd = useQuery(['backend-deps'], () => import('@sd/assets/deps/backend-deps.json'));
	const platform = usePlatform();

	return (
		<div className="custom-scroll page-scroll app-background flex h-screen w-full flex-col p-5">
			<ScreenHeading>Dependencies</ScreenHeading>

			{/* item has a LOT more data that we can display, i just went with the basics */}

			<ScreenHeading className="mb-2">Frontend Dependencies</ScreenHeading>
			<div className="grid gap-6 space-x-1 xl:grid-cols-4 2xl:grid-cols-6">
				{frontEnd.data &&
					frontEnd.data?.default.map((item) => {
						return (
							<a key={item.title} onClick={() => platform.openLink(item.url ?? '')}>
								<div className="rounded border-2 border-gray-500 px-4 py-4 text-gray-300">
									<h4 className="text-center">
										{item.title.trimEnd().substring(0, 24) + (item.title.length > 24 ? '...' : '')}
									</h4>
								</div>
							</a>
						);
					})}
			</div>

			<ScreenHeading className="mb-2">Backend Dependencies</ScreenHeading>
			<div className="grid gap-6 space-x-1 lg:grid-cols-7">
				{backEnd.data &&
					backEnd.data?.default.map((item) => {
						return (
							<a key={item.title} onClick={() => platform.openLink(item.url ?? '')}>
								<div className="rounded border-2 border-gray-500 px-4 py-4 text-gray-300">
									<h4 className="text-center">{item.title.trimEnd()}</h4>
								</div>
							</a>
						);
					})}
			</div>
		</div>
	);
}
