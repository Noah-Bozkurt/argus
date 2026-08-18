export type CommandStatus =
  | 'QUEUED'
  | 'ACCEPTED'
  | 'RUNNING'
  | 'SUCCEEDED'
  | 'FAILED'
  | 'UNKNOWN'
  | 'EXPIRED'

export type RiskLevel = 'LOW' | 'MEDIUM' | 'HIGH'

export type ServiceRestartCommand = {
  kind: 'service.restart'
  service: string
}
