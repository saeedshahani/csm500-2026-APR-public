// Bounded pilot admission, not a currency cap. Evaluate inside store.update.
// Reservations are derived from immutable accepted jobs, including failed/cancelled jobs.
import { fail, terminal } from './store.mjs';

export const pilotLimits = Object.freeze({
  projectsPerOwner: 3,
  outstandingPerUser: 2,
  jobsPerUserDay: 20,
  userDailyExecutionMs: 120000,
  globalDailyExecutionMs: 600000,
  globalMonthlyExecutionMs: 3600000,
});

export function admitProject(state, uid) {
  if (state.projects.filter(project => project.owner_uid === uid).length >= pilotLimits.projectsPerOwner)
    fail(429, 'owner_project_limit', 'Project ownership limit reached');
}

export function admitOutstanding(state, uid, excludedId) {
  if (state.jobs.filter(job => job.id !== excludedId && job.submitted_by === uid && !terminal.has(job.status)).length >= pilotLimits.outstandingPerUser)
    fail(429, 'outstanding_job_limit', 'Cancel or finish existing jobs before submitting or resuming more');
}

export function admitJob(state, uid, settings, now = new Date()) {
  if (!Number.isSafeInteger(settings.deadline_ms) || settings.deadline_ms < 1 || settings.deadline_ms > 30000)
    throw Error('Admission requires validated job settings');
  const date = now.toISOString(), day = date.slice(0, 10), month = date.slice(0, 7);
  let userDay = 0, dayTotal = 0, monthTotal = 0, userJobs = 0;
  admitOutstanding(state, uid);
  for (const job of state.jobs) {
    // Corrupt reservation metadata must not silently become free capacity.
    if (typeof job.created_at !== 'string' || !Number.isFinite(Date.parse(job.created_at))
        || new Date(job.created_at).toISOString() !== job.created_at
        || !Number.isSafeInteger(job.settings?.deadline_ms) || job.settings.deadline_ms < 1 || job.settings.deadline_ms > 30000)
      throw Error('Invalid job admission reservation');
    // Future-dated records are charged conservatively, including after a clock rollback.
    if (job.created_at.slice(0, 7) >= month) monthTotal += job.settings.deadline_ms;
    if (job.created_at.slice(0, 10) >= day) {
      dayTotal += job.settings.deadline_ms;
      if (job.submitted_by === uid) { userDay += job.settings.deadline_ms; userJobs++; }
    }
  }
  if (userJobs >= pilotLimits.jobsPerUserDay || userDay + settings.deadline_ms > pilotLimits.userDailyExecutionMs)
    fail(429, 'user_daily_limit', 'Daily job allowance reached');
  if (dayTotal + settings.deadline_ms > pilotLimits.globalDailyExecutionMs
      || monthTotal + settings.deadline_ms > pilotLimits.globalMonthlyExecutionMs)
    fail(429, 'platform_execution_limit', 'Platform execution allowance reached');
}
