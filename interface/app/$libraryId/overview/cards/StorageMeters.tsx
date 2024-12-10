import { useEffect, useState } from 'react';
import { CircularProgress } from '@sd/ui';
import { useIsDark, useLocale } from '~/hooks';

const StorageMeters = () => {
	const [mounted, setMounted] = useState(false);
	const isDark = useIsDark();
	const { t } = useLocale();

	// Fake stats for demonstration
	const stats = {
		totalUsed: 72, // 72% of storage used
		redundantData: 45, // 45% redundant data
		compressible: 30 // 30% potentially compressible
	};

	useEffect(() => {
		setMounted(true);
	}, []);

	const trackColor = isDark ? '#252631' : '#efefef';

	return (
		<div className="flex flex-col gap-4 p-2">
			<div className="mx-8 flex items-center justify-between gap-4">
				<div className="flex flex-col items-center">
					<CircularProgress
						radius={40}
						progress={mounted ? stats.totalUsed : 0}
						strokeWidth={6}
						trackStrokeWidth={6}
						strokeColor={
							stats.totalUsed >= 90
								? '#E14444'
								: stats.totalUsed >= 75
									? 'darkorange'
									: stats.totalUsed >= 60
										? 'yellow'
										: '#2599FF'
						}
						fillColor="transparent"
						trackStrokeColor={trackColor}
						strokeLinecap="square"
						className="flex items-center justify-center"
						transition="stroke-dashoffset 1s ease 0s, stroke 1s ease"
					>
						<div className="absolute text-lg font-semibold">{stats.totalUsed}%</div>
					</CircularProgress>
					<span className="mt-2 text-sm text-ink-faint">Storage Used</span>
				</div>

				<div className="flex flex-col items-center">
					<CircularProgress
						radius={40}
						progress={mounted ? stats.redundantData : 0}
						strokeWidth={6}
						trackStrokeWidth={6}
						strokeColor="#FF6B6B"
						fillColor="transparent"
						trackStrokeColor={trackColor}
						strokeLinecap="square"
						className="flex items-center justify-center"
						transition="stroke-dashoffset 1s ease 0s, stroke 1s ease"
					>
						<div className="absolute text-lg font-semibold">{stats.redundantData}%</div>
					</CircularProgress>
					<span className="mt-2 text-sm text-ink-faint">Redundant</span>
				</div>

				<div className="flex flex-col items-center">
					<CircularProgress
						radius={40}
						progress={mounted ? stats.compressible : 0}
						strokeWidth={6}
						trackStrokeWidth={6}
						strokeColor="#4CAF50"
						fillColor="transparent"
						trackStrokeColor={trackColor}
						strokeLinecap="square"
						className="flex items-center justify-center"
						transition="stroke-dashoffset 1s ease 0s, stroke 1s ease"
					>
						<div className="absolute text-lg font-semibold">{stats.compressible}%</div>
					</CircularProgress>
					<span className="mt-2 text-sm text-ink-faint">Compressible</span>
				</div>
			</div>
		</div>
	);
};

export default StorageMeters;
