import { Gear } from 'phosphor-react';
import { useClientContext, useDebugState } from '@sd/client';
import { Button, ButtonLink, Popover, Tooltip } from '@sd/ui';
import { dialogManager } from '@sd/ui';
import DebugPopover from './DebugPopover';
import FeedbackDialog from './FeedbackDialog';
import { IsRunningJob, JobsManager } from './JobManager';

export default () => {
	const { library } = useClientContext();
	const debugState = useDebugState();

	return (
		<div className="space-y-2">
			<div className="flex w-full items-center justify-between">
				<div className="flex">
					<ButtonLink
						to="settings/client/general"
						size="icon"
						variant="subtle"
						className="text-ink-faint ring-offset-sidebar"
					>
						<Tooltip label="Settings">
							<Gear className="h-5 w-5" />
						</Tooltip>
					</ButtonLink>
					<Popover
						trigger={
							<Button
								size="icon"
								variant="subtle"
								className="text-ink-faint ring-offset-sidebar radix-state-open:bg-sidebar-selected/50"
								disabled={!library}
							>
								{library && (
									<Tooltip label="Recent Jobs">
										<IsRunningJob />
									</Tooltip>
								)}
							</Button>
						}
					>
						<div className="block h-96 w-[430px]">
							<JobsManager />
						</div>
					</Popover>
				</div>
				<Button
					variant="outline"
					className="flex items-center gap-1"
					onClick={() => {
						dialogManager.create((dp) => <FeedbackDialog {...dp} />);
					}}
				>
					<p className="text-[11px] font-normal text-ink-faint">Feedback</p>
				</Button>
			</div>
			{debugState.enabled && <DebugPopover />}
		</div>
	);
};
