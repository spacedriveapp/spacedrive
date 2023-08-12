import clsx from 'clsx';
import { useNavigate } from 'react-router';
import { useOnboardingStore } from '@sd/client';
import routes from '.';

export default function OnboardingProgress(props: { currentScreen?: string }) {
	const obStore = useOnboardingStore();
	const navigate = useNavigate();

	return (
		<div className="flex w-full items-center justify-center">
			<div className="flex items-center justify-center space-x-1">
				{routes.map(({ path }) => {
					if (!path) return null;

					return (
						<button
							key={path}
							disabled={!obStore.unlockedScreens.includes(path)}
							onClick={() => navigate(`./${path}`, { replace: true })}
							className={clsx(
								'h-2 w-2 rounded-full transition hover:bg-ink disabled:opacity-10',
								props.currentScreen === path ? 'bg-ink' : 'bg-ink-faint'
							)}
						/>
					);
				})}
			</div>
		</div>
	);
}
