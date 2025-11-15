import { motion } from "framer-motion";
import clsx from "clsx";
import { GitBranch, Clock } from "@phosphor-icons/react";
import FolderIcon from "@sd/assets/icons/Folder.png";
import CodeIcon from "@sd/assets/icons/Code-20.png";
import DocumentIcon from "@sd/assets/icons/Document.png";
import ImageIcon from "@sd/assets/icons/Image.png";

interface ProjectCardsProps {
	locations: any[];
}

// MOCK: Smart project detection (future feature)
interface SmartProject {
	id: string;
	name: string;
	path: string;
	type: "git" | "npm" | "cargo" | "xcode" | "generic";
	languages: string[];
	fileTypes: {
		code: number;
		docs: number;
		images: number;
		other: number;
	};
	totalFiles: number;
	totalSize: number;
	lastActivity: Date;
	recentFileCount: number; // Files modified in last 24h
	isGitRepo: boolean;
	gitBranch?: string;
}

// MOCK DATA - This simulates what smart detection would return
function mockSmartProjects(locations: any[]): SmartProject[] {
	const projectTypes = ["git", "npm", "cargo", "xcode", "generic"] as const;
	const languages = [
		["TypeScript", "Rust"],
		["JavaScript", "Python"],
		["Swift", "Objective-C"],
		["Rust", "TOML"],
		["TypeScript", "CSS", "HTML"],
		["Python", "Jupyter"],
	];

	return locations.slice(0, 8).map((loc, idx) => ({
		id: loc.id,
		name: loc.name || `Project ${idx + 1}`,
		path: loc.path,
		type: projectTypes[idx % projectTypes.length],
		languages: languages[idx % languages.length],
		fileTypes: {
			code: Math.floor(Math.random() * 500) + 100,
			docs: Math.floor(Math.random() * 50) + 10,
			images: Math.floor(Math.random() * 100) + 20,
			other: Math.floor(Math.random() * 200) + 50,
		},
		totalFiles: Math.floor(Math.random() * 10000) + 1000,
		totalSize: Math.floor(Math.random() * 1024 * 1024 * 1024) + 1024 * 1024 * 100,
		lastActivity: new Date(Date.now() - Math.random() * 7 * 24 * 60 * 60 * 1000),
		recentFileCount: Math.floor(Math.random() * 50) + 5,
		isGitRepo: idx % 2 === 0,
		gitBranch: idx % 2 === 0 ? ["main", "develop", "feature/new-ui"][idx % 3] : undefined,
	}));
}

export function ProjectCards({ locations }: ProjectCardsProps) {
	const projects = mockSmartProjects(locations);

	return (
		<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
			<div className="px-6 py-4 border-b border-app-line flex items-center justify-between">
				<div>
					<h2 className="text-base font-semibold text-ink">Recent Projects</h2>
					<p className="text-sm text-ink-dull mt-1">
						Your most active workspaces
						<span className="ml-2 px-2 py-0.5 bg-sidebar-box text-sidebar-ink-dull text-xs rounded-md font-medium border border-sidebar-line">
							PREVIEW
						</span>
					</p>
				</div>
				<button className="text-sm text-accent hover:underline font-medium">
					View All →
				</button>
			</div>

			<div className="p-6">
				<div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
					{projects.map((project, idx) => (
						<ProjectCard key={project.id} project={project} index={idx} />
					))}
				</div>

				{projects.length === 0 && (
					<div className="text-center py-12 text-ink-faint">
						<Folder className="size-12 mx-auto mb-3 opacity-20" />
						<p className="text-sm">No projects found</p>
						<p className="text-xs mt-1">Add locations to see your projects here</p>
					</div>
				)}
			</div>
		</div>
	);
}

interface ProjectCardProps {
	project: SmartProject;
	index: number;
}

function ProjectCard({ project, index }: ProjectCardProps) {

	return (
		<motion.button
			initial={{ opacity: 0, y: 20 }}
			animate={{ opacity: 1, y: 0 }}
			transition={{ delay: index * 0.05 }}
			whileHover={{ y: -4, scale: 1.02 }}
			className="group relative flex flex-col gap-3 p-5 bg-app border border-app-line rounded-xl hover:border-accent hover:bg-app-hover transition-all text-left overflow-hidden"
		>
			<div className="relative">
				{/* Icon and badge */}
				<div className="flex items-start justify-between mb-3">
					<div className="p-3 bg-sidebar-box rounded-lg group-hover:bg-sidebar-selected transition-colors">
						<img src={FolderIcon} alt="Folder" className="size-8" />
					</div>

					{project.isGitRepo && (
						<div className="flex items-center gap-1.5 px-2 py-1 bg-sidebar-box rounded-md border border-sidebar-line">
							<GitBranch className="size-3 text-sidebar-ink" weight="bold" />
							<span className="text-xs text-sidebar-ink-dull font-mono">
								{project.gitBranch}
							</span>
						</div>
					)}
				</div>

				{/* Project name */}
				<h3 className="font-semibold text-ink mb-1 truncate">
					{project.name}
				</h3>

				{/* Languages */}
				<div className="flex flex-wrap gap-1 mb-3">
					{project.languages.slice(0, 3).map((lang) => (
						<span
							key={lang}
							className="px-2 py-0.5 bg-sidebar-box text-sidebar-ink-dull text-xs rounded border border-sidebar-line"
						>
							{lang}
						</span>
					))}
					{project.languages.length > 3 && (
						<span className="px-2 py-0.5 bg-sidebar-box text-sidebar-ink-faint text-xs rounded border border-sidebar-line">
							+{project.languages.length - 3}
						</span>
					)}
				</div>

				{/* File breakdown */}
				<div className="grid grid-cols-2 gap-2 mb-3 text-xs">
					<div className="flex items-center gap-1.5">
						<img src={CodeIcon} alt="Code" className="size-4 opacity-60" />
						<span className="text-ink-dull">
							{project.fileTypes.code} code
						</span>
					</div>
					<div className="flex items-center gap-1.5">
						<img src={DocumentIcon} alt="Docs" className="size-4 opacity-60" />
						<span className="text-ink-dull">
							{project.fileTypes.docs} docs
						</span>
					</div>
					<div className="flex items-center gap-1.5">
						<img src={ImageIcon} alt="Images" className="size-4 opacity-60" />
						<span className="text-ink-dull">
							{project.fileTypes.images} images
						</span>
					</div>
					<div className="flex items-center gap-1.5">
						<img src={FolderIcon} alt="Other" className="size-4 opacity-60" />
						<span className="text-ink-dull">
							{project.fileTypes.other} other
						</span>
					</div>
				</div>

				{/* Total files */}
				<div className="text-sm text-ink-dull mb-2">
					{project.totalFiles.toLocaleString()} files total
				</div>

				{/* Recent activity */}
				<div className="flex items-center gap-1.5 text-xs">
					<Clock className="size-3 text-ink-dull" weight="bold" />
					<span className="text-ink-dull font-medium">
						Active {formatRelativeTime(project.lastActivity)}
					</span>
					<span className="text-ink-faint">
						• {project.recentFileCount} files
					</span>
				</div>
			</div>
		</motion.button>
	);
}


function formatRelativeTime(date: Date): string {
	const now = Date.now();
	const diff = now - date.getTime();
	const minutes = Math.floor(diff / 60000);
	const hours = Math.floor(diff / 3600000);
	const days = Math.floor(diff / 86400000);

	if (minutes < 60) return `${minutes}m ago`;
	if (hours < 24) return `${hours}h ago`;
	if (days < 7) return `${days}d ago`;
	return `${Math.floor(days / 7)}w ago`;
}
