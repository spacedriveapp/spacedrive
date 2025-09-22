#!/usr/bin/env node

/**
 * Simple test script for the TypeScript client
 */

import { SpacedriveClient, Event, JobStatus } from './index';

async function main() {
  console.log('🚀 Testing Spacedrive TypeScript Client...');

  const client = new SpacedriveClient();

  try {
    // Test ping
    console.log('\n🏓 Testing ping...');
    await client.ping();

    // Test queries
    console.log('\n📚 Testing library list...');
    const libraries = await client.getLibraries(false);
    console.log(`Found ${libraries.length} libraries:`, libraries);

    console.log('\n💼 Testing job list...');
    const jobs = await client.getJobs();
    console.log(`Found ${jobs.jobs.length} jobs:`, jobs.jobs);

    // Test event subscription
    console.log('\n🎧 Testing event subscription...');
    await client.subscribe(['JobStarted', 'JobProgress', 'JobCompleted']);

    client.on('spacedrive-event', (event: Event) => {
      console.log('📡 Received event:', event);

      // Type-safe event handling
      if (typeof event === 'string') {
        console.log(`📡 Simple event: ${event}`);
      } else if ('JobStarted' in event) {
        console.log(`🚀 Job started: ${event.JobStarted.job_type} (${event.JobStarted.job_id})`);
      } else if ('JobProgress' in event) {
        const progress = Math.round(event.JobProgress.progress * 100);
        console.log(`📊 Job progress: ${event.JobProgress.job_type} - ${progress}%`);

        if (event.JobProgress.generic_progress) {
          console.log(`  📈 Phase: ${event.JobProgress.generic_progress.phase}`);
          console.log(`  💬 Message: ${event.JobProgress.generic_progress.message}`);
        }
      } else if ('JobCompleted' in event) {
        console.log(`✅ Job completed: ${event.JobCompleted.job_type}`);
        console.log('📋 Output:', event.JobCompleted.output);
      }
    });

    client.on('error', (error) => {
      console.error('❌ Client error:', error);
    });

    client.on('disconnected', () => {
      console.log('🔌 Disconnected from daemon');
    });

    // Keep the script running to receive events
    console.log('✅ All tests completed! Listening for events... (Ctrl+C to exit)');

  } catch (error) {
    console.error('❌ Test failed:', error);
    process.exit(1);
  }
}

main().catch(console.error);
