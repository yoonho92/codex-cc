use super::*;

impl Session {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub async fn request_mcp_server_elicitation(
        &self,
        turn_context: &TurnContext,
        request_id: RequestId,
        params: McpServerElicitationRequestParams,
    ) -> Option<ElicitationResponse> {
        let server_name = params.server_name.clone();
        let request = match params.request {
            McpServerElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            } => {
                let requested_schema = match serde_json::to_value(requested_schema) {
                    Ok(requested_schema) => requested_schema,
                    Err(err) => {
                        warn!(
                            "failed to serialize MCP elicitation schema for server_name: {server_name}, request_id: {request_id}: {err:#}"
                        );
                        return None;
                    }
                };
                codex_protocol::approvals::ElicitationRequest::Form {
                    meta,
                    message,
                    requested_schema,
                }
            }
            McpServerElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            } => codex_protocol::approvals::ElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            },
        };

        let (tx_response, rx_response) = oneshot::channel();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_elicitation(
                        server_name.clone(),
                        request_id.clone(),
                        tx_response,
                    )
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!(
                "Overwriting existing pending elicitation for server_name: {server_name}, request_id: {request_id}"
            );
        }
        let id = match request_id {
            rmcp::model::NumberOrString::String(value) => {
                codex_protocol::mcp::RequestId::String(value.to_string())
            }
            rmcp::model::NumberOrString::Number(value) => {
                codex_protocol::mcp::RequestId::Integer(value)
            }
        };
        let event = EventMsg::ElicitationRequest(ElicitationRequestEvent {
            turn_id: params.turn_id,
            server_name,
            id,
            request,
        });
        self.send_event(turn_context, event).await;
        rx_response.await.ok()
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and manager fallback must stay serialized"
    )]
    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> anyhow::Result<()> {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_elicitation(&server_name, &id)
                }
                None => None,
            }
        };
        if let Some(tx_response) = entry {
            tx_response
                .send(response)
                .map_err(|e| anyhow::anyhow!("failed to send elicitation response: {e:?}"))?;
            return Ok(());
        }

        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_elicitation(server_name, id, response)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourcesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resources(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourceTemplatesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resource_templates(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> anyhow::Result<ReadResourceResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .read_resource(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP tool calls are serialized through the session-owned manager guard"
    )]
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub(crate) fn mcp_logging_notification_handler(
        self: &Arc<Self>,
    ) -> McpLoggingNotificationHandler {
        let session = Arc::downgrade(self);
        Arc::new(move |server_name, params| {
            let session = session.clone();
            async move {
                let Some(session) = session.upgrade() else {
                    return;
                };
                session
                    .handle_mcp_logging_notification_as_channel(server_name, params)
                    .await;
            }
            .boxed()
        })
    }

    async fn handle_mcp_logging_notification_as_channel(
        self: &Arc<Self>,
        server_name: String,
        params: LoggingMessageNotificationParam,
    ) {
        let Some(message) = channel_message_from_mcp_logging(&server_name, params) else {
            return;
        };
        let item = message.item;

        if let Err(err) = self.try_ensure_rollout_materialized().await {
            warn!("failed to materialize rollout for MCP channel message: {err}");
            return;
        }

        self.send_event_raw(Event {
            id: item.id.clone(),
            msg: item.as_legacy_event(),
        })
        .await;

        if let Err(err) = self.flush_rollout().await {
            warn!("failed to flush MCP channel message rollout: {err}");
        }

        self.queue_channel_message_for_next_turn(&item, message.model_text.as_deref())
            .await;
        self.maybe_start_turn_for_pending_work().await;
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP tool metadata reads through the session-owned manager guard"
    )]
    pub(crate) async fn resolve_mcp_tool_info(&self, tool_name: &ToolName) -> Option<ToolInfo> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_tool_info(tool_name)
            .await
    }

    async fn refresh_mcp_servers_inner(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        let auth = self.services.auth_manager.auth().await;
        let config = self.get_config().await;
        let mcp_config = config
            .to_mcp_config(self.services.plugins_manager.as_ref())
            .await;
        let tool_plugin_provenance = self
            .services
            .mcp_manager
            .tool_plugin_provenance(config.as_ref())
            .await;
        let mcp_servers = with_codex_apps_mcp(mcp_servers, auth.as_ref(), &mcp_config);
        let auth_statuses =
            compute_auth_statuses(mcp_servers.iter(), store_mode, auth.as_ref()).await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            guard.cancel();
            *guard = CancellationToken::new();
        }
        let (refreshed_manager, cancel_token) = McpConnectionManager::new(
            &mcp_servers,
            store_mode,
            auth_statuses,
            &turn_context.approval_policy,
            turn_context.sub_id.clone(),
            self.get_tx_event(),
            turn_context.sandbox_policy.get().clone(),
            McpRuntimeEnvironment::new(
                turn_context
                    .environment
                    .clone()
                    .unwrap_or_else(|| self.services.environment_manager.local_environment()),
                turn_context.cwd.to_path_buf(),
            ),
            config.codex_home.to_path_buf(),
            codex_apps_tools_cache_key(auth.as_ref()),
            tool_plugin_provenance,
            Some(self.mcp_logging_notification_handler()),
            auth.as_ref(),
        )
        .await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            if guard.is_cancelled() {
                cancel_token.cancel();
            }
            *guard = cancel_token;
        }

        let mut manager = self.services.mcp_connection_manager.write().await;
        *manager = refreshed_manager;
    }

    pub(crate) async fn refresh_mcp_servers_if_requested(
        self: &Arc<Self>,
        turn_context: &TurnContext,
    ) {
        let refresh_config = { self.pending_mcp_server_refresh_config.lock().await.take() };
        let Some(refresh_config) = refresh_config else {
            return;
        };

        let McpServerRefreshConfig {
            mcp_servers,
            mcp_oauth_credentials_store_mode,
        } = refresh_config;

        let mcp_servers =
            match serde_json::from_value::<HashMap<String, McpServerConfig>>(mcp_servers) {
                Ok(servers) => servers,
                Err(err) => {
                    warn!("failed to parse MCP server refresh config: {err}");
                    return;
                }
            };
        let store_mode = match serde_json::from_value::<OAuthCredentialsStoreMode>(
            mcp_oauth_credentials_store_mode,
        ) {
            Ok(mode) => mode,
            Err(err) => {
                warn!("failed to parse MCP OAuth refresh config: {err}");
                return;
            }
        };

        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_now(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
    }

    #[cfg(test)]
    pub(crate) async fn mcp_startup_cancellation_token(&self) -> CancellationToken {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .clone()
    }

    pub(crate) async fn cancel_mcp_startup(&self) {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .cancel();
    }
}

struct ParsedChannelMessage {
    item: ChannelMessageItem,
    model_text: Option<String>,
}

fn channel_message_from_mcp_logging(
    server_name: &str,
    params: LoggingMessageNotificationParam,
) -> Option<ParsedChannelMessage> {
    let LoggingMessageNotificationParam {
        level,
        logger,
        data,
    } = params;

    if logger.as_deref() != Some("codex_channel") {
        return None;
    }

    let Value::Object(map) = data else {
        return None;
    };

    if !map
        .get("codexChannelMessage")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    let text = object_string(&map, "text")?;
    if text.trim().is_empty() {
        return None;
    }

    let id = object_string(&map, "id")
        .or_else(|| object_string(&map, "itemId"))
        .or_else(|| object_string(&map, "msg_id"))
        .or_else(|| object_string(&map, "msgId"))
        .unwrap_or_else(|| format!("mcp_channel_{}", Uuid::new_v4()));
    let channel = object_string(&map, "channel").unwrap_or_else(|| "mcp".to_string());
    let sender = object_string(&map, "sender")
        .or_else(|| object_string(&map, "from"))
        .unwrap_or_else(|| server_name.to_string());
    let sender_kind = object_string(&map, "senderKind")
        .as_deref()
        .map(parse_channel_sender_kind)
        .unwrap_or(ChannelSenderKind::External);
    let priority = object_string(&map, "priority")
        .as_deref()
        .map(parse_channel_priority)
        .unwrap_or_else(|| priority_from_mcp_log_level(level));
    let delivery = object_string(&map, "delivery")
        .as_deref()
        .map(parse_channel_delivery)
        .unwrap_or_default();
    let created_at_ms =
        object_i64(&map, "createdAtMs").unwrap_or_else(|| Utc::now().timestamp_millis());

    Some(ParsedChannelMessage {
        item: ChannelMessageItem {
            id,
            channel,
            sender,
            sender_kind,
            text,
            preview: object_string(&map, "preview"),
            priority,
            delivery,
            created_at_ms,
        },
        model_text: object_string(&map, "modelText").or_else(|| object_string(&map, "model_text")),
    })
}

fn object_string(map: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn object_i64(map: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| value.try_into().ok()))
    })
}

fn parse_channel_sender_kind(value: &str) -> ChannelSenderKind {
    match value {
        "user" => ChannelSenderKind::User,
        "agent" => ChannelSenderKind::Agent,
        "system" => ChannelSenderKind::System,
        _ => ChannelSenderKind::External,
    }
}

fn parse_channel_priority(value: &str) -> ChannelPriority {
    match value {
        "low" => ChannelPriority::Low,
        "high" | "critical" => ChannelPriority::High,
        _ => ChannelPriority::Normal,
    }
}

fn priority_from_mcp_log_level(level: LoggingLevel) -> ChannelPriority {
    match level {
        LoggingLevel::Emergency
        | LoggingLevel::Alert
        | LoggingLevel::Critical
        | LoggingLevel::Error
        | LoggingLevel::Warning => ChannelPriority::High,
        LoggingLevel::Debug => ChannelPriority::Low,
        LoggingLevel::Notice | LoggingLevel::Info => ChannelPriority::Normal,
    }
}

fn parse_channel_delivery(value: &str) -> ChannelDelivery {
    match value {
        "surfaceAndQueueNextTurn" | "surface_and_queue_next_turn" => {
            ChannelDelivery::SurfaceAndQueueNextTurn
        }
        _ => ChannelDelivery::SurfaceOnly,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn mcp_channel_logging_notification_builds_channel_item() {
        let message = channel_message_from_mcp_logging(
            "codex_cc2cc",
            LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("codex_channel".to_string()),
                data: json!({
                    "codexChannelMessage": true,
                    "id": "msg-1",
                    "channel": "cc2cc",
                    "sender": "codex-peer",
                    "senderKind": "agent",
                    "text": "hello",
                    "priority": "high",
                    "delivery": "surfaceOnly",
                    "createdAtMs": 123,
                }),
            },
        )
        .expect("marked notification should be promoted");

        let item = message.item;
        assert_eq!(item.id, "msg-1");
        assert_eq!(item.channel, "cc2cc");
        assert_eq!(item.sender, "codex-peer");
        assert_eq!(item.sender_kind, ChannelSenderKind::Agent);
        assert_eq!(item.text, "hello");
        assert_eq!(item.priority, ChannelPriority::High);
        assert_eq!(item.created_at_ms, 123);
    }

    #[test]
    fn mcp_channel_logging_notification_keeps_model_text_separate() {
        let message = channel_message_from_mcp_logging(
            "telegram",
            LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("codex_channel".to_string()),
                data: json!({
                    "codexChannelMessage": true,
                    "id": "msg-2",
                    "channel": "telegram",
                    "sender": "alice",
                    "text": "surface preview only",
                    "modelText": "handle this through the telegram tool",
                    "delivery": "surfaceAndQueueNextTurn",
                    "createdAtMs": 456,
                }),
            },
        )
        .expect("marked notification should be promoted");

        assert_eq!(message.item.text, "surface preview only");
        assert_eq!(
            message.model_text.as_deref(),
            Some("handle this through the telegram tool")
        );
    }

    #[test]
    fn channel_next_turn_prompt_is_generic_and_uses_model_text() {
        let item = ChannelMessageItem {
            id: "msg-3".to_string(),
            channel: "telegram".to_string(),
            sender: "alice".to_string(),
            sender_kind: ChannelSenderKind::External,
            text: "surface preview only".to_string(),
            preview: Some("surface preview only".to_string()),
            priority: ChannelPriority::Normal,
            delivery: ChannelDelivery::SurfaceAndQueueNextTurn,
            created_at_ms: 789,
        };

        let queued = channel_message_response_item(&item, Some("call the telegram receive tool"))
            .expect("queueable channel message");
        let ResponseInputItem::Message { role, content, .. } = queued else {
            panic!("expected message response input item");
        };
        assert_eq!(role, "developer");
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one text content item");
        };
        assert!(text.contains("\"channel\": \"telegram\""));
        assert!(text.contains("\"has_model_text\": true"));
        assert!(text.contains("call the telegram receive tool"));
        assert!(!text.contains("cc2cc"));
    }

    #[test]
    fn channel_next_turn_prompt_without_model_text_omits_display_text() {
        let item = ChannelMessageItem {
            id: "msg-4".to_string(),
            channel: "external-chat".to_string(),
            sender: "telegram".to_string(),
            sender_kind: ChannelSenderKind::External,
            text: "raw remote body that should stay out of model context".to_string(),
            preview: Some("short preview".to_string()),
            priority: ChannelPriority::Normal,
            delivery: ChannelDelivery::SurfaceAndQueueNextTurn,
            created_at_ms: 789,
        };

        let queued = channel_message_response_item(&item, None).expect("metadata-only prompt");
        let ResponseInputItem::Message { content, .. } = queued else {
            panic!("expected message response input item");
        };
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one text content item");
        };
        assert!(text.contains("\"channel\": \"external-chat\""));
        assert!(text.contains("\"has_model_text\": false"));
        assert!(!text.contains("raw remote body"));
        assert!(!text.contains("short preview"));
    }

    #[test]
    fn mcp_channel_logging_notification_ignores_unmarked_logs() {
        let item = channel_message_from_mcp_logging(
            "other_server",
            LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("other_logger".to_string()),
                data: json!({
                    "text": "not a channel message",
                }),
            },
        );

        assert!(item.is_none());
    }
}
