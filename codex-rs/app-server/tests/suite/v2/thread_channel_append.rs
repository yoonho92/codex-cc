use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ChannelDelivery;
use codex_app_server_protocol::ChannelMessageAppendedNotification;
use codex_app_server_protocol::ChannelPriority;
use codex_app_server_protocol::ChannelSenderKind;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadChannelAppendParams;
use codex_app_server_protocol::ThreadChannelAppendResponse;
use codex_app_server_protocol::ThreadChannelMessageInput;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[tokio::test]
async fn thread_channel_append_emits_notification_and_persists_channel_item() -> Result<()> {
    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let thread_req = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_req)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(thread_resp)?;

    let message = ThreadChannelMessageInput {
        id: Some("channel-1".to_string()),
        channel: "peer-inbox".to_string(),
        sender: "codex-peer".to_string(),
        sender_kind: ChannelSenderKind::Agent,
        text: "hello from peer".to_string(),
        preview: Some("hello".to_string()),
        priority: ChannelPriority::High,
        delivery: ChannelDelivery::SurfaceOnly,
        model_text: None,
        created_at_ms: Some(1_717_171_717_000),
    };

    let append_req = mcp
        .send_thread_channel_append_request(ThreadChannelAppendParams {
            thread_id: thread.id.clone(),
            message: message.clone(),
        })
        .await?;
    let append_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(append_req)),
    )
    .await??;
    let response: ThreadChannelAppendResponse =
        to_response::<ThreadChannelAppendResponse>(append_resp)?;
    assert!(response.accepted);
    assert_eq!(response.item_id, "channel-1");

    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/channel/appended"),
    )
    .await??;
    let server_notification = ServerNotification::try_from(notification)?;
    let ServerNotification::ChannelMessageAppended(ChannelMessageAppendedNotification {
        thread_id,
        item,
    }) = server_notification
    else {
        panic!("expected thread/channel/appended notification");
    };
    assert_eq!(thread_id, thread.id);
    assert_eq!(
        item,
        ThreadItem::ChannelMessage {
            id: "channel-1".to_string(),
            channel: "peer-inbox".to_string(),
            sender: "codex-peer".to_string(),
            sender_kind: ChannelSenderKind::Agent,
            text: "hello from peer".to_string(),
            preview: Some("hello".to_string()),
            priority: ChannelPriority::High,
            delivery: ChannelDelivery::SurfaceOnly,
            created_at_ms: 1_717_171_717_000,
        }
    );

    let read_req = mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id: thread.id.clone(),
            include_turns: true,
        })
        .await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_req)),
    )
    .await??;
    let ThreadReadResponse { thread, .. } = to_response::<ThreadReadResponse>(read_resp)?;
    assert_eq!(thread.turns.len(), 1);
    assert_eq!(thread.turns[0].items.len(), 1);
    assert_eq!(
        thread.turns[0].items[0],
        ThreadItem::ChannelMessage {
            id: "channel-1".to_string(),
            channel: "peer-inbox".to_string(),
            sender: "codex-peer".to_string(),
            sender_kind: ChannelSenderKind::Agent,
            text: "hello from peer".to_string(),
            preview: Some("hello".to_string()),
            priority: ChannelPriority::High,
            delivery: ChannelDelivery::SurfaceOnly,
            created_at_ms: 1_717_171_717_000,
        }
    );

    Ok(())
}

fn create_config_toml(codex_home: &Path, server_uri: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
