/**
 * Protection dashboard
 *
 * Shows library-wide replication score, per-volume redundancy bars,
 * and an at-risk data callout.
 */

import {ArrowRight, ShieldCheck, Warning} from '@phosphor-icons/react';
import {motion} from 'framer-motion';
import {useMemo} from 'react';
import {useNavigate} from 'react-router-dom';
import {useLibraryQuery} from '../../contexts/SpacedriveContext';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {RedundancyVolumeBar} from './components/RedundancyVolumeBar';

function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function RedundancyDashboard() {
	const navigate = useNavigate();

	const {data, isLoading} = useLibraryQuery({
		type: 'redundancy.summary',
		input: {}
	});

	const scorePercent = useMemo(() => {
		if (!data) return 0;
		return Math.round(data.library_totals.replication_score * 100);
	}, [data]);

	const scoreColor =
		scorePercent >= 75
			? 'text-status-success'
			: scorePercent >= 40
				? 'text-status-warning'
				: 'text-status-error';

	const topBarTitle = useMemo(
		() => (
			<div className="flex items-center gap-2">
				<ShieldCheck size={20} weight="bold" className="text-ink" />
				<h1 className="text-ink text-xl font-bold">Protection</h1>
			</div>
		),
		[]
	);

	if (isLoading || !data) {
		return (
			<>
				<TopBarPortal
					left={
						<TopBarItem
							id="redundancy-title"
							label="Protection"
							priority="high"
						>
							{topBarTitle}
						</TopBarItem>
					}
				/>
				<div className="text-ink-dull flex h-full items-center justify-center">
					Loading protection data...
				</div>
			</>
		);
	}

	const {library_totals, volumes} = data;
	const totalAtRiskFiles = volumes.reduce(
		(sum, v) => sum + v.at_risk_file_count,
		0
	);

	return (
		<>
			<TopBarPortal
				left={
					<TopBarItem
						id="redundancy-title"
						label="Protection"
						priority="high"
					>
						{topBarTitle}
					</TopBarItem>
				}
			/>

			<div className="flex h-full flex-col overflow-hidden">
				<div className="flex-1 space-y-4 overflow-auto p-4">
					{/* Replication Score + At-Risk Summary */}
					<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
						{/* Replication Score Card */}
						<div className="border-app-line bg-app-box/50 flex items-center gap-4 rounded-lg border p-4">
							<div className="flex flex-col items-center">
								<motion.span
									className={`text-4xl font-bold ${scoreColor}`}
									initial={{opacity: 0, scale: 0.5}}
									animate={{opacity: 1, scale: 1}}
									transition={{duration: 0.4}}
								>
									{scorePercent}%
								</motion.span>
								<span className="text-ink-dull text-xs">
									Replication Score
								</span>
							</div>
							<div className="text-ink-dull flex-1 space-y-1 text-xs">
								<div>
									<span className="text-ink">
										{formatBytes(
											library_totals.total_redundant_bytes
										)}
									</span>{' '}
									safely replicated
								</div>
								<div>
									<span className="text-ink">
										{formatBytes(
											library_totals.total_at_risk_bytes
										)}
									</span>{' '}
									at risk (single copy)
								</div>
								<div>
									<span className="text-ink">
										{formatBytes(
											library_totals.total_unique_content_bytes
										)}
									</span>{' '}
									unique content total
								</div>
							</div>
						</div>

						{/* At-Risk Callout */}
						<button
							onClick={() => navigate('/redundancy/at-risk')}
							className="border-status-warning/30 bg-status-warning/5 hover:bg-status-warning/10 flex items-center gap-4 rounded-lg border p-4 text-left transition-colors"
						>
							<Warning
								size={32}
								weight="fill"
								className="text-status-warning flex-shrink-0"
							/>
							<div className="flex-1">
								<div className="text-ink text-sm font-medium">
									{totalAtRiskFiles.toLocaleString()} files at
									risk
								</div>
								<div className="text-ink-dull text-xs">
									{formatBytes(
										library_totals.total_at_risk_bytes
									)}{' '}
									of data exists on only one volume
								</div>
							</div>
							<ArrowRight
								size={16}
								className="text-ink-dull flex-shrink-0"
							/>
						</button>
					</div>

					{/* Per-Volume Redundancy Bars */}
					<div>
						<h2 className="text-ink-dull mb-2 text-sm font-medium">
							Per-Volume Breakdown
						</h2>
						<div className="space-y-2">
							{volumes.length === 0 ? (
								<div className="border-app-line bg-app-box/50 text-ink-dull rounded-lg border p-6 text-center text-sm">
									No volumes with indexed content found. Track
									storage to see its protection state.
								</div>
							) : (
								volumes.map((vol) => (
									<RedundancyVolumeBar
										key={vol.volume_uuid}
										volumeUuid={vol.volume_uuid}
										displayName={vol.display_name}
										totalBytes={vol.total_bytes}
										atRiskBytes={vol.at_risk_bytes}
										atRiskFileCount={vol.at_risk_file_count}
										redundantBytes={vol.redundant_bytes}
										redundantFileCount={
											vol.redundant_file_count
										}
										totalFileCount={vol.total_file_count}
									/>
								))
							)}
						</div>
					</div>

					{/* Quick Actions */}
					<div className="flex gap-2">
						<button
							onClick={() => navigate('/redundancy/compare')}
							className="border-app-line bg-app-box/50 text-ink hover:bg-app-hover rounded-lg border px-4 py-2 text-sm transition-colors"
						>
							Compare Volumes
						</button>
						<button
							onClick={() => navigate('/redundancy/at-risk')}
							className="border-app-line bg-app-box/50 text-ink hover:bg-app-hover rounded-lg border px-4 py-2 text-sm transition-colors"
						>
							View At-Risk Files
						</button>
					</div>
				</div>
			</div>
		</>
	);
}
