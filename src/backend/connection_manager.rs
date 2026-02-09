use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::backend::client::BackendClient;
use crate::config::BackendConfig;

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Connecting,
}

/// Backend connection manager with auto-reconnect
pub struct BackendConnectionManager {
    config: BackendConfig,
    client: Arc<RwLock<Option<BackendClient>>>,
    state: Arc<RwLock<ConnectionState>>,
}

impl BackendConnectionManager {
    /// Create a new connection manager
    pub fn new(config: BackendConfig) -> Self {
        Self {
            config,
            client: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
        }
    }

    /// Get the current client (if connected)
    pub fn client(&self) -> Arc<RwLock<Option<BackendClient>>> {
        self.client.clone()
    }

    /// Get the current connection state
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Try to connect to backend
    async fn try_connect(&self) -> Result<BackendClient> {
        tracing::info!("Attempting to connect to backend at {}...", self.config.url);
        
        let client = BackendClient::new(
            &self.config.url,
            self.config.frontend_id.clone()
        ).await?;
        
        tracing::info!("Successfully connected to backend");
        Ok(client)
    }

    /// Initial connection attempt (non-blocking)
    pub async fn initialize(&self) {
        *self.state.write().await = ConnectionState::Connecting;
        
        match self.try_connect().await {
            Ok(client) => {
                *self.client.write().await = Some(client);
                *self.state.write().await = ConnectionState::Connected;
                tracing::info!("Backend connection established");
            }
            Err(e) => {
                *self.state.write().await = ConnectionState::Disconnected;
                tracing::warn!("Failed to connect to backend: {}. Will retry in background.", e);
            }
        }
    }

    /// Start auto-reconnect task
    pub fn start_reconnect_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let reconnect_interval = Duration::from_secs(10); // Retry every 10 seconds
            
            loop {
                sleep(reconnect_interval).await;
                
                let state = self.state.read().await.clone();
                
                // Only try to reconnect if disconnected
                if state == ConnectionState::Disconnected {
                    tracing::debug!("Backend disconnected, attempting reconnection...");
                    
                    *self.state.write().await = ConnectionState::Connecting;
                    
                    match self.try_connect().await {
                        Ok(client) => {
                            *self.client.write().await = Some(client);
                            *self.state.write().await = ConnectionState::Connected;
                            tracing::info!("Backend reconnected successfully");
                        }
                        Err(e) => {
                            *self.state.write().await = ConnectionState::Disconnected;
                            tracing::debug!("Reconnection failed: {}. Will retry in {}s", 
                                e, reconnect_interval.as_secs());
                        }
                    }
                }
            }
        });
    }

    /// Mark connection as disconnected (called when operations fail)
    pub async fn mark_disconnected(&self) {
        let current_state = self.state.read().await.clone();
        
        if current_state == ConnectionState::Connected {
            tracing::warn!("Backend connection lost, entering offline mode");
            *self.state.write().await = ConnectionState::Disconnected;
            *self.client.write().await = None;
        }
    }

    /// Start heartbeat task with auto-reconnect
    pub fn start_heartbeat_task(self: Arc<Self>) {
        let heartbeat_interval = self.config.heartbeat_interval;
        
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(heartbeat_interval)).await;
                
                let state = self.state.read().await.clone();
                
                if state == ConnectionState::Connected {
                    if let Some(client) = &mut *self.client.write().await {
                        let mut status = std::collections::HashMap::new();
                        status.insert("status".to_string(), "healthy".to_string());
                        
                        match client.heartbeat(status).await {
                            Ok(response) => {
                                if response.healthy {
                                    tracing::debug!("Heartbeat sent successfully");
                                } else {
                                    tracing::warn!("Backend reported unhealthy: {}", response.message);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Heartbeat failed: {}", e);
                                self.mark_disconnected().await;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Start command subscription task with auto-reconnect
    pub fn start_command_subscription_task(
        self: Arc<Self>,
        platforms: Vec<crate::utils::Platform>,
    ) {
        tokio::spawn(async move {
            loop {
                let state = self.state.read().await.clone();
                
                if state == ConnectionState::Connected {
                    tracing::info!("Subscribing to backend commands...");

                    let mut subscription_client = match BackendClient::new(
                        &self.config.url,
                        self.config.frontend_id.clone(),
                    )
                    .await
                    {
                        Ok(client) => client,
                        Err(e) => {
                            tracing::error!("Failed to create subscription client: {}", e);
                            self.mark_disconnected().await;
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    };

                    match subscription_client.subscribe_commands(platforms.clone()).await {
                        Ok(mut stream) => {
                            tracing::info!("Subscribed to backend commands");

                            // Process commands until stream ends or error
                            loop {
                                match stream.message().await {
                                    Ok(Some(command)) => {
                                        tracing::info!(
                                            "Received command from backend: {} (ID: {})",
                                            command.command_type,
                                            command.command_id
                                        );

                                        // TODO: Dispatch command to appropriate handler
                                        match command.command_type.as_str() {
                                            "send_message" => {
                                                tracing::info!("Send message command: {:?}", command.parameters);
                                            }
                                            "shutdown" => {
                                                tracing::warn!("Received shutdown command from backend");
                                                // TODO: Graceful shutdown
                                            }
                                            _ => {
                                                tracing::debug!("Unhandled command type: {}", command.command_type);
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        tracing::warn!("Backend command stream ended");
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::error!("Error receiving command: {}", e);
                                        break;
                                    }
                                }
                            }

                            // Stream ended, mark as disconnected
                            self.mark_disconnected().await;
                        }
                        Err(e) => {
                            tracing::error!("Failed to subscribe to commands: {}", e);
                            self.mark_disconnected().await;
                        }
                    }
                }
                
                // Wait before retrying subscription (will be connected by reconnect task)
                sleep(Duration::from_secs(5)).await;
            }
        });
    }
}
