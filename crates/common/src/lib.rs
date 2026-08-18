use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Duration, Utc};
use protocol::{Command, CommandRequest, CommandResult, CommandStatus, CommandType};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub client_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Environment {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Server {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Service {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub server_id: Uuid,
    pub name: String,
    pub service_type: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub actor: String,
    pub resource: String,
    pub action: String,
    pub request_id: String,
    pub result: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum DomainEvent {
    #[serde(rename = "server.connected")]
    ServerConnected { server_id: Uuid },
    #[serde(rename = "server.disconnected")]
    ServerDisconnected { server_id: Uuid },
    #[serde(rename = "server.command.started")]
    ServerCommandStarted { server_id: Uuid, command_id: Uuid },
    #[serde(rename = "server.command.completed")]
    ServerCommandCompleted { server_id: Uuid, command_id: Uuid },
    #[serde(rename = "server.command.failed")]
    ServerCommandFailed { server_id: Uuid, command_id: Uuid },
    #[serde(rename = "project.created")]
    ProjectCreated { project_id: Uuid },
}

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("command already exists for idempotency key: {0}")]
    Duplicate(String),
    #[error("operation conflict for server {server_id}")]
    Conflict { server_id: Uuid },
    #[error("invalid ttl")]
    InvalidTtl,
    #[error("command not found")]
    NotFound,
}

#[derive(Debug, Clone)]
pub struct CommandQueue {
    queues: HashMap<Uuid, VecDeque<Command>>,
    running_conflicts: HashMap<Uuid, HashSet<String>>,
    command_index: HashMap<Uuid, Command>,
    command_server: HashMap<Uuid, Uuid>,
    idempotency_index: HashMap<(Uuid, String), Uuid>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
            running_conflicts: HashMap::new(),
            command_index: HashMap::new(),
            command_server: HashMap::new(),
            idempotency_index: HashMap::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        request: CommandRequest,
        now: DateTime<Utc>,
    ) -> Result<Command, QueueError> {
        if request.ttl_seconds <= 0 {
            return Err(QueueError::InvalidTtl);
        }

        let key = (request.server_id, request.idempotency_key.clone());
        if let Some(existing) = self.idempotency_index.get(&key) {
            return Err(QueueError::Duplicate(existing.to_string()));
        }

        if self.has_conflict(request.server_id, &request.command_type) {
            return Err(QueueError::Conflict {
                server_id: request.server_id,
            });
        }

        let command = Command {
            id: Uuid::new_v4(),
            server_id: request.server_id,
            command_type: request.command_type,
            created_at: now,
            expires_at: now + Duration::seconds(request.ttl_seconds),
            status: CommandStatus::QUEUED,
            idempotency_key: request.idempotency_key,
            risk_level: request.risk_level,
        };

        self.queues
            .entry(command.server_id)
            .or_default()
            .push_back(command.clone());
        self.command_server.insert(command.id, command.server_id);
        self.command_index.insert(command.id, command.clone());
        self.idempotency_index.insert(
            (command.server_id, command.idempotency_key.clone()),
            command.id,
        );

        Ok(command)
    }

    pub fn next_for_server(&mut self, server_id: Uuid, now: DateTime<Utc>) -> Option<Command> {
        let queue = self.queues.get_mut(&server_id)?;

        while let Some(mut command) = queue.pop_front() {
            if command.expires_at < now {
                command.status = CommandStatus::EXPIRED;
                self.command_index.insert(command.id, command);
                continue;
            }

            command.status = CommandStatus::ACCEPTED;
            self.running_conflicts
                .entry(server_id)
                .or_default()
                .insert(command.command_type.conflict_group().to_string());
            self.command_index.insert(command.id, command.clone());
            return Some(command);
        }

        None
    }

    pub fn mark_running(&mut self, command_id: Uuid) -> Result<(), QueueError> {
        let Some(command) = self.command_index.get_mut(&command_id) else {
            return Err(QueueError::NotFound);
        };

        command.status = CommandStatus::RUNNING;
        Ok(())
    }

    pub fn complete(&mut self, result: CommandResult) -> Result<(), QueueError> {
        let Some(command) = self.command_index.get_mut(&result.command_id) else {
            return Err(QueueError::NotFound);
        };

        command.status = result.status;

        if let Some(server_id) = self.command_server.get(&result.command_id)
            && let Some(running) = self.running_conflicts.get_mut(server_id)
        {
            running.remove(command.command_type.conflict_group());
        }

        Ok(())
    }

    pub fn get(&self, command_id: Uuid) -> Option<&Command> {
        self.command_index.get(&command_id)
    }

    fn has_conflict(&self, server_id: Uuid, command_type: &CommandType) -> bool {
        self.running_conflicts
            .get(&server_id)
            .is_some_and(|running| running.contains(command_type.conflict_group()))
            || self.queues.get(&server_id).is_some_and(|queue| {
                queue.iter().any(|queued| {
                    queued.command_type.conflict_group() == command_type.conflict_group()
                })
            })
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{CommandRequest, RiskLevel};

    #[test]
    fn project_client_is_optional() {
        let project = Project {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            name: "Argus".to_string(),
            client_id: None,
        };

        assert!(project.client_id.is_none());
    }

    #[test]
    fn queue_rejects_duplicate_idempotency_key() {
        let mut queue = CommandQueue::new();
        let now = Utc::now();
        let server_id = Uuid::new_v4();
        let request = CommandRequest {
            server_id,
            command_type: CommandType::ServiceRestart {
                service: "nginx.service".to_string(),
            },
            ttl_seconds: 60,
            idempotency_key: "same-key".to_string(),
            risk_level: RiskLevel::MEDIUM,
        };

        queue.enqueue(request.clone(), now).expect("first enqueue");
        let error = queue
            .enqueue(request, now)
            .expect_err("must reject duplicate");
        assert!(matches!(error, QueueError::Duplicate(_)));
    }

    #[test]
    fn queue_rejects_conflicting_operations() {
        let mut queue = CommandQueue::new();
        let now = Utc::now();
        let server_id = Uuid::new_v4();
        queue
            .enqueue(
                CommandRequest {
                    server_id,
                    command_type: CommandType::ServiceRestart {
                        service: "nginx.service".to_string(),
                    },
                    ttl_seconds: 60,
                    idempotency_key: "k1".to_string(),
                    risk_level: RiskLevel::MEDIUM,
                },
                now,
            )
            .expect("enqueue");

        let error = queue
            .enqueue(
                CommandRequest {
                    server_id,
                    command_type: CommandType::ServiceRestart {
                        service: "docker.service".to_string(),
                    },
                    ttl_seconds: 60,
                    idempotency_key: "k2".to_string(),
                    risk_level: RiskLevel::MEDIUM,
                },
                now,
            )
            .expect_err("must conflict");

        assert!(matches!(error, QueueError::Conflict { .. }));
    }

    #[test]
    fn expired_commands_do_not_execute() {
        let mut queue = CommandQueue::new();
        let now = Utc::now();
        let server_id = Uuid::new_v4();
        queue
            .enqueue(
                CommandRequest {
                    server_id,
                    command_type: CommandType::ServiceRestart {
                        service: "nginx.service".to_string(),
                    },
                    ttl_seconds: 1,
                    idempotency_key: "exp".to_string(),
                    risk_level: RiskLevel::MEDIUM,
                },
                now,
            )
            .expect("enqueue");

        let command = queue.next_for_server(server_id, now + Duration::seconds(2));
        assert!(command.is_none());
    }
}
