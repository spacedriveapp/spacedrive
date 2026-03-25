import {
	Atom,
	Brain,
	CalendarDots,
	CaretDown,
	ChatCircleDots,
	Checks,
	ClockCounterClockwise,
	DotsThree,
	Microphone,
	Sparkle,
	Robot,
} from "@phosphor-icons/react";
import { Ball, BallBlue } from "@sd/assets/images";
import { Popover, SearchBar, usePopover } from "@sd/ui";
import { useEffect, useState } from "react";
import { usePlatform } from "../contexts/PlatformContext";

const isMacOS =
	typeof navigator !== "undefined" &&
	(navigator.platform.toLowerCase().includes("mac") || navigator.userAgent.includes("Mac"));

const primaryItems = [
	{ icon: ChatCircleDots, label: "Chat", active: true },
	{ icon: Checks, label: "Tasks", active: false },
	{ icon: Brain, label: "Memories", active: false },
	{ icon: Atom, label: "Autonomy", active: false },
	{ icon: CalendarDots, label: "Schedule", active: false },
];

const projects = [
	{ name: "Spacedrive v3", detail: "Main workspace", ball: BallBlue },
	{ name: "Spacebot Runtime", detail: "Remote control plane", ball: Ball },
	{ name: "Hosted Platform", detail: "Deploy and observe", ball: Ball },
];

const history = [
	{ title: "Weekly planning", meta: "Today, 9:41 AM" },
	{ title: "Fix onboarding flow", meta: "Yesterday" },
	{ title: "Review launch copy", meta: "Mon" },
	{ title: "Debug worker memory", meta: "Sun" },
];

const agents = [
	{ name: "James", detail: "Founder mode" },
	{ name: "Operations", detail: "Scheduling and triage" },
	{ name: "Builder", detail: "Code and tooling" },
];

const projectOptions = ["Spacedrive v3", "Spacebot Runtime", "Hosted Platform"];
const modelOptions = ["Claude 3.7 Sonnet", "GPT-5", "Qwen 2.5 72B"];

export function Spacebot() {
	const platform = usePlatform();
	const [search, setSearch] = useState("");
	const agentSelector = usePopover();
	const composerProjectSelector = usePopover();
	const composerModelSelector = usePopover();

	useEffect(() => {
		if (platform.applyMacOSStyling) {
			platform.applyMacOSStyling().catch((error) => {
				console.warn("Failed to apply macOS styling:", error);
			});
		}
	}, [platform]);

	function openVoiceOverlay() {
		if (!platform.showWindow) return;
		platform.showWindow({ type: "VoiceOverlay" }).catch((error) => {
			console.warn("Failed to open voice overlay:", error);
		});
	}

	return (
		<div className="relative h-full overflow-hidden bg-app text-ink">
			<div
				data-tauri-drag-region
				className="top-bar-blur absolute inset-x-0 top-0 z-20 flex h-12 items-center justify-between border-b border-app-line bg-app/85 px-3"
				style={{ paddingLeft: isMacOS ? 92 : 12 }}
			>
				<div className="flex items-center gap-3" data-tauri-drag-region>
					<div className="w-[156px]" data-tauri-drag-region>
						<Popover
							popover={agentSelector}
							trigger={
								<button className="flex h-8 w-full items-center gap-2 rounded-full border border-sidebar-line/30 bg-sidebar-box/20 px-3 text-left text-xs font-medium text-sidebar-inkDull backdrop-blur-xl transition-all hover:bg-sidebar-box/30 hover:text-sidebar-ink active:scale-95">
									<span className="flex-1 truncate text-left">James</span>
									<CaretDown className="size-3" weight="bold" />
								</button>
							}
							align="start"
							sideOffset={8}
							className="min-w-[180px] p-2"
						>
							<div className="space-y-1">
								{agents.map((agent) => (
									<button
										key={agent.name}
										onClick={() => agentSelector.setOpen(false)}
										className="w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-app-selected"
									>
										<div>
											<div className="font-medium">{agent.name}</div>
											<div className="text-xs text-ink-dull">{agent.detail}</div>
										</div>
									</button>
								))}
							</div>
						</Popover>
					</div>
				</div>

				<div className="flex items-center gap-2" data-tauri-drag-region>
					<SearchBar
						value={search}
						onChange={setSearch}
						placeholder="Search"
						className="w-64"
					/>
					<button className="rounded-full border border-app-line bg-accent px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-accent-faint">
						New chat
					</button>
				</div>
			</div>

			<div className="flex h-full pt-12">
				<aside className="flex w-[280px] shrink-0 flex-col border-r border-app-line bg-sidebar">
					<nav className="space-y-1 px-3 py-3">
						{primaryItems.map((item) => {
							const Icon = item.icon;
							return (
								<button
									key={item.label}
									className={`flex w-full flex-row items-center gap-0.5 truncate rounded-lg px-2 py-1.5 text-left text-sm font-medium tracking-wide outline-none transition-colors ring-inset ring-transparent focus:ring-1 focus:ring-accent ${
										item.active
											? "bg-sidebar-selected/40 text-sidebar-ink"
											: "text-sidebar-inkDull hover:text-sidebar-ink"
									}`}
								>
									<Icon className="mr-2 size-4" weight={item.active ? "fill" : "bold"} />
									<span className="truncate">{item.label}</span>
								</button>
							);
						})}
					</nav>

					<div className="flex-1 overflow-y-auto px-3 pb-4">
						<section className="mb-5">
							<div className="mb-2 flex items-center justify-between px-2">
								<div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-sidebar-inkDull">
									Projects
								</div>
								<DotsThree className="size-4 text-sidebar-inkDull" />
							</div>
							<div className="space-y-1">
								{projects.map((project) => (
									<button
										key={project.name}
										className="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-sidebar-box"
									>
										<img
											src={project.ball}
											alt=""
											className="size-7 shrink-0 object-contain"
											draggable={false}
										/>
										<div>
											<div className="text-sm font-medium text-sidebar-ink">{project.name}</div>
											<div className="text-xs text-sidebar-inkDull">{project.detail}</div>
										</div>
									</button>
								))}
							</div>
						</section>

						<section>
							<div className="mb-2 flex items-center justify-between px-2">
								<div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-sidebar-inkDull">
									History
								</div>
								<ClockCounterClockwise className="size-4 text-sidebar-inkDull" />
							</div>
							<div className="space-y-1">
								{history.map((item) => (
									<button
										key={item.title}
										className="flex w-full items-center rounded-lg px-3 py-2 text-left transition-colors hover:bg-sidebar-box"
									>
										<div>
											<div className="text-sm font-medium text-sidebar-ink">{item.title}</div>
											<div className="text-xs text-sidebar-inkDull">{item.meta}</div>
										</div>
									</button>
								))}
							</div>
						</section>
					</div>
				</aside>

				<main className="relative flex min-w-0 flex-1 items-center justify-center overflow-hidden bg-app p-6">
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0 opacity-100"
						style={{
							backgroundImage:
								"linear-gradient(to right, hsla(var(--color-app-line), 0.45) 1px, transparent 1px), linear-gradient(to bottom, hsla(var(--color-app-line), 0.45) 1px, transparent 1px)",
							backgroundSize: "28px 28px",
							maskImage: "linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))",
							WebkitMaskImage: "linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))",
						}}
					/>
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0"
						style={{
							background:
								"radial-gradient(circle at top, hsla(var(--color-accent), 0.08), transparent 42%)",
						}}
					/>

					<div className="w-full max-w-3xl">
						<div className="mb-6 text-left">
							<h1 className="text-[2.65rem] font-semibold tracking-tight text-ink">
								Let&apos;s get to work, James
							</h1>
							<p className="mt-2 text-sm text-ink-dull">
								Learn how to be productive with Spacebot. {""}
								<a
									href="https://github.com/spacedriveapp/spacebot"
									target="_blank"
									rel="noreferrer"
									className="text-ink-dull underline underline-offset-4 transition-colors hover:text-ink"
								>
									Read the docs.
								</a>
							</p>
						</div>

						<div className="rounded-[28px] border border-app-line bg-app-box/90 p-4 shadow-[0_30px_80px_rgba(0,0,0,0.25)] backdrop-blur-xl">
							<div className="mb-3 flex items-center gap-2 px-1 text-xs font-medium text-ink-dull">
								<Sparkle className="size-3.5 text-accent" weight="fill" />
								What should Spacebot work on?
							</div>

							<div className="rounded-[24px] border border-app-line bg-app p-4">
								<textarea
									placeholder="Ask Spacebot to review a project, plan work, or start a task..."
									className="min-h-[140px] w-full resize-none border-0 bg-transparent text-base leading-7 text-ink outline-none placeholder:text-ink-faint focus:border-0 focus:outline-none focus:ring-0"
								/>

								<div className="mt-4 flex items-center justify-between gap-3">
									<div className="w-[210px]">
										<Popover
											popover={composerProjectSelector}
											trigger={
												<button className="flex h-9 w-full items-center gap-2 rounded-full border border-app-line bg-app-box px-3 text-left text-xs font-medium text-ink-dull transition-colors hover:bg-app-hover hover:text-ink">
													<span className="flex-1 truncate text-left">Spacedrive v3</span>
													<CaretDown className="size-3" weight="bold" />
												</button>
											}
											align="start"
											sideOffset={8}
											className="min-w-[220px] p-2"
										>
											<div className="space-y-1">
												{projectOptions.map((project) => (
													<button
														key={project}
														onClick={() => composerProjectSelector.setOpen(false)}
														className="w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-app-selected"
													>
														{project}
													</button>
												))}
											</div>
										</Popover>
									</div>

									<div className="flex items-center gap-2">
										<div className="w-[180px]">
											<Popover
												popover={composerModelSelector}
												trigger={
													<button className="flex h-9 w-full items-center gap-2 rounded-full border border-app-line bg-app-box px-3 text-left text-xs font-medium text-ink-dull transition-colors hover:bg-app-hover hover:text-ink">
														<span className="flex-1 truncate text-left">Claude 3.7 Sonnet</span>
														<CaretDown className="size-3" weight="bold" />
													</button>
												}
												align="end"
												sideOffset={8}
												className="min-w-[220px] p-2"
											>
												<div className="space-y-1">
													{modelOptions.map((model) => (
														<button
															key={model}
															onClick={() => composerModelSelector.setOpen(false)}
															className="w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm text-ink transition-colors hover:bg-app-selected"
														>
															{model}
														</button>
													))}
												</div>
											</Popover>
										</div>

										<button
											onClick={openVoiceOverlay}
											className="flex size-9 items-center justify-center rounded-full border border-app-line bg-app-box text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
										>
											<Microphone className="size-4" weight="fill" />
										</button>
									</div>
								</div>
							</div>
						</div>
					</div>
				</main>
			</div>
		</div>
	);
}
