import React from 'react';
import { useBridgeQuery } from '@sd/client';
import ProgressBar from '../primitive/ProgressBar';
import { Transition } from '@headlessui/react';

export default function RunningJobsWidget() {
  const { data: jobs } = useBridgeQuery('JobGetRunning');

  return (
    <div className="flex flex-col space-y-4">
      {jobs?.map((job) => (
        <Transition
          show={true}
          enter="transition-translate ease-in-out duration-200"
          enterFrom="translate-y-24"
          enterTo="translate-y-0"
          leave="transition-translate ease-in-out duration-200"
          leaveFrom="translate-y-0"
          leaveTo="translate-y-24"
        >
          <div key={job.id} className="flex flex-col px-2 pt-1.5 pb-2 bg-gray-700 rounded">
            {/* <span className="mb-0.5 text-tiny font-bold text-gray-400">{job.status} Job</span> */}
            <span className="mb-1.5 truncate text-gray-450 text-tiny">{job.message}</span>
            <ProgressBar value={job.completed_task_count} total={job.task_count} />
          </div>
        </Transition>
      ))}
    </div>
  );
}
