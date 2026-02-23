pub const MODULE_NAME: &str = "aivi.chronos.scheduler";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.chronos.scheduler
export Trigger, CronSpec, Job, JobStatus, PlannedRun, PlanKey, PlanIndex
export Lease, RetryPolicy, RetryJitter, RetryState, TenantLimit
export MetricEvent, LogEvent
export cron, interval, once
export planKey, upsertPlan, hasPlan
export leaseActive, canAcquireLease, heartbeatLease
export retryDelay, retryAt
export countActiveForTenant, canStartForTenant
export metricScheduled, metricStarted, metricRetried, metricCompleted
export logScheduled, logLease, logRetry, logCompleted
export domain Scheduler

use aivi
use aivi.chronos.instant (Timestamp)
use aivi.chronos.instant (domain Instant)
use aivi.chronos.duration (Span)
use aivi.chronos.timezone (TimeZone)

CronSpec = {
  expression: Text
  timezone: TimeZone
}

Trigger =
  | Cron CronSpec
  | Interval Span
  | Once Timestamp

Job = {
  jobId: Text
  tenantId: Text
  trigger: Trigger
  maxAttempts: Int
  leaseTtl: Span
  heartbeatEvery: Span
}

JobStatus = Planned | Leased | Running | Succeeded | Failed | DeadLetter

PlanKey = {
  jobId: Text
  scheduledAt: Timestamp
}

PlannedRun = {
  key: PlanKey
  tenantId: Text
  trigger: Trigger
  scheduledAt: Timestamp
  attempt: Int
  status: JobStatus
}

PlanIndex = Map Text PlannedRun

Lease = {
  runKey: PlanKey
  ownerId: Text
  leasedAt: Timestamp
  leaseUntil: Timestamp
  heartbeatEvery: Span
}

RetryJitter = NoJitter | PlusMinusPermille Int

RetryPolicy =
  | NoRetry
  | Fixed Span RetryJitter
  | Exponential {
      base: Span
      cap: Span
      jitter: RetryJitter
    }

RetryState = {
  attempts: Int
  lastError: Option Text
}

TenantLimit = {
  tenantId: Text
  maxConcurrent: Int
}

MetricEvent = {
  name: Text
  tenantId: Text
  jobId: Text
  runAt: Timestamp
  value: Int
  tags: Map Text Text
}

LogEvent = {
  level: Text
  message: Text
  fields: Map Text Text
}

cron : Text -> TimeZone -> Trigger
cron = expression timezone => Cron { expression, timezone }

interval : Span -> Trigger
interval = span => Interval span

once : Timestamp -> Trigger
once = timestamp => Once timestamp

planKey : Text -> Timestamp -> PlanKey
planKey = jobId scheduledAt => { jobId, scheduledAt }

planKeyText : PlanKey -> Text
planKeyText = key => text.concat [key.jobId, "@", text.toText key.scheduledAt]

upsertPlan : PlannedRun -> PlanIndex -> PlanIndex
upsertPlan = run index => Map.insert (planKeyText run.key) run index

hasPlan : PlanKey -> PlanIndex -> Bool
hasPlan = key index => Map.has (planKeyText key) index

leaseActive : Timestamp -> Lease -> Bool
leaseActive = now lease => now < lease.leaseUntil

canAcquireLease : Timestamp -> Option Lease -> Bool
canAcquireLease = now leaseOpt => leaseOpt match
  | None => True
  | Some lease => not (leaseActive now lease)

heartbeatLease : Lease -> Timestamp -> Lease
heartbeatLease = lease heartbeatAt => {
  runKey: lease.runKey
  ownerId: lease.ownerId
  leasedAt: heartbeatAt
  leaseUntil: heartbeatAt + (lease.leaseUntil - lease.leasedAt)
  heartbeatEvery: lease.heartbeatEvery
}

pow2 : Int -> Int
pow2 = exponent =>
  if exponent <= 0 then 1 else 2 * pow2 (exponent - 1)

policyJitter : RetryPolicy -> RetryJitter
policyJitter = policy => policy match
  | NoRetry => NoJitter
  | Fixed _ jitter => jitter
  | Exponential cfg => cfg.jitter

baseDelayMillis : RetryPolicy -> Int -> Int
baseDelayMillis = policy attempt =>
  policy match
    | NoRetry => 0
    | Fixed span _ => span.millis
    | Exponential cfg => {
        n = if attempt <= 1 then 0 else attempt - 1
        raw = cfg.base.millis * pow2 n
        if raw > cfg.cap.millis then cfg.cap.millis else raw
      }

clampPermille : Int -> Int
clampPermille = value =>
  if value < 0 then 0 else if value > 1000 then 1000 else value

applyJitter : Int -> RetryJitter -> Int -> Int
applyJitter = millis jitter jitterSeed =>
  jitter match
    | NoJitter => millis
    | PlusMinusPermille spread => {
        boundedSpread = clampPermille spread
        centered = clampPermille jitterSeed - 500
        jitterRange = (millis * boundedSpread) / 1000
        bumped = millis + ((jitterRange * centered) / 500)
        if bumped < 0 then 0 else bumped
      }

retryDelay : RetryPolicy -> Int -> Int -> Span
retryDelay = policy attempt jitterSeed => {
  millis: applyJitter (baseDelayMillis policy attempt) (policyJitter policy) jitterSeed
}

retryAt : Timestamp -> RetryPolicy -> Int -> Int -> Timestamp
retryAt = at policy attempt jitterSeed => at + retryDelay policy attempt jitterSeed

isActiveStatus : JobStatus -> Bool
isActiveStatus = status => status match
  | Leased => True
  | Running => True
  | _ => False

countActiveForTenant : Text -> List PlannedRun -> Int
countActiveForTenant = tenantId runs => runs match
  | [] => 0
  | [run, ...rest] =>
      if run.tenantId == tenantId && isActiveStatus run.status
      then 1 + countActiveForTenant tenantId rest
      else countActiveForTenant tenantId rest

canStartForTenant : TenantLimit -> List PlannedRun -> Bool
canStartForTenant = limit runs =>
  countActiveForTenant limit.tenantId runs < limit.maxConcurrent

metric : Text -> Text -> Text -> Timestamp -> Int -> Map Text Text -> MetricEvent
metric = name tenantId jobId runAt value tags => { name, tenantId, jobId, runAt, value, tags }

metricScheduled : Text -> Text -> Timestamp -> MetricEvent
metricScheduled = tenantId jobId runAt =>
  metric "scheduler.run.scheduled" tenantId jobId runAt 1 (Map.fromList [("phase", "planning")])

metricStarted : Text -> Text -> Timestamp -> MetricEvent
metricStarted = tenantId jobId runAt =>
  metric "scheduler.run.started" tenantId jobId runAt 1 (Map.fromList [("phase", "lease")])

metricRetried : Text -> Text -> Timestamp -> Int -> MetricEvent
metricRetried = tenantId jobId runAt attempt =>
  metric "scheduler.run.retried" tenantId jobId runAt attempt (Map.fromList [("phase", "retry")])

metricCompleted : Text -> Text -> Timestamp -> Bool -> MetricEvent
metricCompleted = tenantId jobId runAt succeeded =>
  metric "scheduler.run.completed" tenantId jobId runAt 1 (Map.fromList [("succeeded", text.toText succeeded)])

log : Text -> Text -> Map Text Text -> LogEvent
log = level message fields => { level, message, fields }

logScheduled : PlannedRun -> LogEvent
logScheduled = run =>
  log "info" "planned scheduler run" (Map.fromList [
    ("jobId", run.key.jobId),
    ("tenantId", run.tenantId),
    ("scheduledAt", text.toText run.scheduledAt)
  ])

logLease : Lease -> LogEvent
logLease = lease =>
  log "debug" "lease heartbeat" (Map.fromList [
    ("jobId", lease.runKey.jobId),
    ("ownerId", lease.ownerId),
    ("leaseUntil", text.toText lease.leaseUntil)
  ])

logRetry : PlannedRun -> Int -> Span -> LogEvent
logRetry = run attempt delay =>
  log "warn" "scheduler retry planned" (Map.fromList [
    ("jobId", run.key.jobId),
    ("tenantId", run.tenantId),
    ("attempt", text.toText attempt),
    ("delayMillis", text.toText delay.millis)
  ])

logCompleted : PlannedRun -> Bool -> LogEvent
logCompleted = run ok =>
  log "info" "scheduler run completed" (Map.fromList [
    ("jobId", run.key.jobId),
    ("tenantId", run.tenantId),
    ("ok", text.toText ok)
  ])

domain Scheduler over JobStatus = {
  isTerminal : JobStatus -> Bool
  isTerminal = status => status match
    | Succeeded => True
    | DeadLetter => True
    | _ => False
}

domain Scheduler over PlannedRun = {
  keyText : PlannedRun -> Text
  keyText = run => planKeyText run.key

  withStatus : PlannedRun -> JobStatus -> PlannedRun
  withStatus = run status => { ...run, status: status }
}
"#;
