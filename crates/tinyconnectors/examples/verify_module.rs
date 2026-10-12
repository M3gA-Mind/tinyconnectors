//! Loads a built module through the real `TinyBus` dynamic loader.

use std::io;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tinybus::Connection;
use tinybus::broker::Broker;
use tinybus::module::{ModuleHost, ModuleInfo};
use tinybus::transport::memory::MemoryBus;
use tinyconnectors::names;
use tinyconnectors_bus::vocabulary::{
    ActionVocabularyResponse, NormalizeIdentitiesResponse, PreparedTaskFilter, VocabularyRequest,
    VocabularyResponse,
};

/// Verify classic behavior and then restricted behavior in a fresh process.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = module_argument()?;
    if std::env::args().nth(2).as_deref() == Some("--restricted") {
        verify_restricted_surface(&module).await?;
        return Ok(());
    }
    let (backend_url, requests) = mock_backend()?;
    let bus = MemoryBus::new();
    let broker = Broker::new();
    let broker_task = broker.spawn(bus.clone());
    let module_host = ModuleHost::new(broker);
    // All verifier operations are local and must work without credentials.
    let state_dir = std::env::current_dir()?.join("target/module-e2e-archive");
    let info = module_host.load_file_with_config(
        &module,
        serde_json::json!({
            "route": "proxy",
            "base_url": backend_url,
            "auth_token": "module-e2e-token",
            "state_dir": state_dir,
        }),
    )?;

    if info.name != env!("CARGO_PKG_NAME") {
        return Err(io::Error::other(format!(
            "loaded module `{}` instead of `{}`",
            info.name,
            env!("CARGO_PKG_NAME")
        ))
        .into());
    }

    verify_served_surface(&bus, &info).await?;
    verify_connector_operations(&bus, &backend_url, &requests).await?;
    // A cdylib owns a process-global tracing subscriber, so a second runtime
    // of the same artifact must be verified in a fresh process.
    let status = std::process::Command::new(std::env::current_exe()?)
        .arg(&module)
        .arg("--restricted")
        .status()?;
    if !status.success() {
        return Err(io::Error::other("restricted module verification failed").into());
    }

    println!(
        "verified {} as TinyBus module `{}`",
        module.display(),
        info.name
    );
    broker_task.abort();
    Ok(())
}

/// Load another instance with an immutable mode and verify deny-by-default dispatch.
async fn verify_restricted_surface(
    module: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bus = MemoryBus::new();
    let broker = Broker::new();
    let task = broker.clone().spawn(bus.clone());
    let host = ModuleHost::new(broker);
    host.load_file_with_config(
        module,
        serde_json::json!({"runtime_mode":"routed_read_only"}),
    )
    .map_err(|error| io::Error::other(format!("restricted initialization: {error}")))?;
    let client = Connection::connect(bus.connect().await?).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client
                .list_names()
                .await?
                .iter()
                .any(|name| name.as_str() == names::INTERFACE)
            {
                return tinybus::Result::Ok(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;
    let mode: tinyconnectors_bus::ComposioRuntimeModeResponse =
        proxy.call(names::methods::RUNTIME_MODE, ()).await?;
    if mode.mode != tinyconnectors_bus::ComposioRuntimeMode::RoutedReadOnly
        || mode.contract_version != tinyconnectors_bus::CONTRACT_VERSION
    {
        return Err(io::Error::other("restricted mode handshake mismatch").into());
    }
    for member in tinyconnectors_bus::METHODS {
        if [
            names::methods::RUNTIME_MODE,
            names::methods::ROUTED_READ,
            names::methods::LIST_CAPABILITIES,
            names::methods::LIST_AGENT_READY_TOOLKITS,
        ]
        .contains(member)
        {
            continue;
        }
        match proxy
            .call::<serde_json::Value>(member, serde_json::json!({"malformed":true}))
            .await
        {
            Err(error) if error.wire_name() == tinybus::Error::UNKNOWN_METHOD => {}
            _ => {
                return Err(
                    io::Error::other("restricted mode accepted an unsupported member").into(),
                );
            }
        }
    }
    let error = proxy
        .call::<serde_json::Value>(
            names::methods::ROUTED_READ,
            serde_json::json!({"malformed":true}),
        )
        .await;
    if !matches!(error, Err(error) if error.wire_name() == tinybus::Error::CONFIDENTIALITY_REQUIRED)
    {
        return Err(io::Error::other("restricted read accepted ordinary delivery").into());
    }
    task.abort();
    Ok(())
}

/// Wait until the module claims its bus name, then check that its manifest
/// declares exactly the members the contract does.
///
/// Preparation and classification use local fixture payloads without calling
/// any backend or requiring a signed-in user.
async fn verify_served_surface(
    bus: &MemoryBus,
    info: &ModuleInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Connection::connect(bus.connect().await?).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let claimed = client.list_names().await?;
            if claimed.iter().any(|name| name.as_str() == names::INTERFACE) {
                return tinybus::Result::Ok(());
            }
            tokio::task::yield_now().await;
        }
    })
    .await??;

    let declared: Vec<String> = info
        .manifest
        .provides
        .iter()
        .flat_map(|interface| interface.methods.iter())
        .map(ToString::to_string)
        .collect();
    let expected: Vec<String> = names::METHODS.iter().map(ToString::to_string).collect();
    if declared != expected {
        return Err(io::Error::other(format!(
            "module declares {declared:?} but the contract declares {expected:?}"
        ))
        .into());
    }
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;
    let prepared: tinyconnectors_bus::PreparedArguments = proxy
        .call(
            names::methods::PREPARE_ARGUMENTS,
            (tinyconnectors_bus::PrepareArgumentsRequest {
                tool: "GOOGLECALENDAR_EVENTS_LIST".into(),
                arguments: Some(serde_json::json!({"timeMin":"2026-06-01"})),
                timezone: Some("Asia/Kuwait".into()),
                since: None,
            },),
        )
        .await?;
    if prepared.error.is_some()
        || prepared
            .arguments
            .as_ref()
            .and_then(|args| args.get("timeZone"))
            != Some(&serde_json::json!("Asia/Kuwait"))
    {
        return Err(io::Error::other("module did not apply calendar defaults").into());
    }
    let classified: tinyconnectors_bus::ProviderError = proxy
        .call(
            names::methods::CLASSIFY_ERROR,
            (tinyconnectors_bus::ClassifyErrorRequest {
                tool: "GMAIL_SEND_EMAIL".into(),
                message: "HTTP 404 connection error, try to authenticate".into(),
            },),
        )
        .await?;
    if classified.class != "action_not_found" {
        return Err(io::Error::other("module did not classify action failure").into());
    }
    verify_vocabulary(&proxy).await?;
    Ok(())
}

/// Drive the backend-facing members through the loaded cdylib and a loopback
/// fixture. No connector service or third-party API is contacted.
async fn verify_connector_operations(
    bus: &MemoryBus,
    backend_url: &str,
    requests: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tinyconnectors_bus::{
        ComposioAuthorizeRequest, ComposioConfigureRequest, ComposioCreateTriggerRequest,
        ComposioDeleteConnectionRequest, ComposioDisableTriggerRequest,
        ComposioEnableTriggerRequest, ComposioExecuteRequest, ComposioGetUserScopesRequest,
        ComposioListAvailableTriggersRequest, ComposioListGithubReposRequest,
        ComposioListToolsRequest, ComposioListTriggersRequest, ComposioSetUserScopesRequest,
        ComposioUserProfileRequest, ComposioUserScopes, names::methods,
    };

    let client = Connection::connect(bus.connect().await?).await?;
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;

    // Configure also exercises a lazy-loaded, initially unconfigured host.
    let configured: tinyconnectors_bus::ComposioConfigureResponse = proxy
        .call(
            methods::CONFIGURE,
            (ComposioConfigureRequest::Proxy {
                base_url: backend_url.to_string(),
                auth_token: "module-e2e-token".into(),
                timezone: Some("Asia/Kuwait".into()),
                transport: None,
            },),
        )
        .await?;
    if configured.route != "proxy" {
        return Err(io::Error::other("Configure did not install the proxy route").into());
    }

    let toolkits: tinyconnectors_bus::ComposioToolkitsResponse =
        proxy.call(methods::LIST_TOOLKITS, ()).await?;
    if !toolkits.toolkits.iter().any(|toolkit| toolkit == "gmail") {
        return Err(io::Error::other("mock backend toolkit was not returned").into());
    }
    let connections: tinyconnectors_bus::ComposioConnectionsResponse =
        proxy.call(methods::LIST_CONNECTIONS, ()).await?;
    if connections.connections.first().map(|row| row.id.as_str()) != Some("conn_1") {
        return Err(io::Error::other("mock backend connection was not returned").into());
    }

    let authorized: tinyconnectors_bus::ComposioAuthorizeResponse = proxy
        .call(
            methods::AUTHORIZE,
            (ComposioAuthorizeRequest {
                toolkit: "gmail".into(),
                extra_params: None,
            },),
        )
        .await?;
    if authorized.connection_id != "conn_new" || !authorized.connect_url.starts_with("https://") {
        return Err(io::Error::other("authorize handoff did not survive the loaded module").into());
    }

    let listed: tinyconnectors_bus::ComposioToolsResponse = proxy
        .call(
            methods::LIST_TOOLS,
            (ComposioListToolsRequest {
                toolkits: vec!["gmail".into()],
                tags: Vec::new(),
                apply_user_scopes: false,
            },),
        )
        .await?;
    if listed.tools.first().map(|tool| tool.function.name.as_str()) != Some("GMAIL_SEND_EMAIL") {
        return Err(io::Error::other("mock backend tool listing was not returned").into());
    }

    let scopes: tinyconnectors_bus::ComposioUserScopesResponse = proxy
        .call(
            methods::SET_USER_SCOPES,
            (ComposioSetUserScopesRequest {
                toolkit: "gmail".into(),
                scopes: ComposioUserScopes {
                    read: true,
                    write: false,
                    admin: false,
                },
            },),
        )
        .await?;
    let persisted: tinyconnectors_bus::ComposioUserScopesResponse = proxy
        .call(
            methods::GET_USER_SCOPES,
            (ComposioGetUserScopesRequest {
                toolkit: "gmail".into(),
            },),
        )
        .await?;
    if scopes.scopes.write || persisted.scopes.write {
        return Err(
            io::Error::other("user scope preference was not enforced in module state").into(),
        );
    }
    let execution_error = proxy
        .call::<tinyconnectors_bus::ComposioExecuteResponse>(
            methods::EXECUTE,
            (ComposioExecuteRequest {
                tool: "GMAIL_SEND_EMAIL".into(),
                arguments: Some(serde_json::json!({"to":"person@example.test"})),
                connection_id: Some("conn_1".into()),
            },),
        )
        .await;
    if !matches!(execution_error, Err(error) if error.to_string().contains("not permitted")) {
        return Err(io::Error::other("Execute did not enforce the stored scope preference").into());
    }
    let reset_scopes: tinyconnectors_bus::ComposioUserScopesResponse = proxy
        .call(
            methods::SET_USER_SCOPES,
            (ComposioSetUserScopesRequest {
                toolkit: "gmail".into(),
                scopes: ComposioUserScopes::default(),
            },),
        )
        .await?;
    if !reset_scopes.scopes.write {
        return Err(io::Error::other("scope reset did not restore write access").into());
    }
    let executed: tinyconnectors_bus::ComposioExecuteResponse = proxy
        .call(
            methods::EXECUTE,
            (ComposioExecuteRequest {
                tool: "GMAIL_SEND_EMAIL".into(),
                arguments: Some(serde_json::json!({"to":"person@example.test"})),
                connection_id: Some("conn_1".into()),
            },),
        )
        .await?;
    if !executed.successful || executed.data["sent"] != true {
        return Err(
            io::Error::other("Execute result was not returned from the mock backend").into(),
        );
    }

    let profile: tinyconnectors_bus::ComposioUserProfile = proxy
        .call(
            methods::GET_USER_PROFILE,
            (ComposioUserProfileRequest {
                toolkit: "gmail".into(),
                connection_id: Some("conn_1".into()),
            },),
        )
        .await?;
    if profile.email.as_deref() != Some("tester@example.test") {
        return Err(io::Error::other("profile action did not return its mock identity").into());
    }
    let refreshed: tinyconnectors_bus::ComposioRefreshIdentitiesResponse =
        proxy.call(methods::REFRESH_ALL_IDENTITIES, ()).await?;
    if refreshed.profiles.is_empty() {
        return Err(io::Error::other("identity refresh did not read active connections").into());
    }

    let repos: tinyconnectors_bus::ComposioGithubReposResponse = proxy
        .call(
            methods::LIST_GITHUB_REPOS,
            (ComposioListGithubReposRequest {
                connection_id: Some("conn_1".into()),
            },),
        )
        .await?;
    if repos.connection_id != "conn_1"
        || repos
            .repositories
            .first()
            .map(|repo| repo.full_name.as_str())
            != Some("tinyhumans/example")
    {
        return Err(
            io::Error::other("GitHub repository listing did not decode the fixture").into(),
        );
    }

    let available: tinyconnectors_bus::ComposioAvailableTriggersResponse = proxy
        .call(
            methods::LIST_AVAILABLE_TRIGGERS,
            (ComposioListAvailableTriggersRequest {
                toolkit: "gmail".into(),
                connection_id: Some("conn_1".into()),
            },),
        )
        .await?;
    if available
        .triggers
        .first()
        .map(|trigger| trigger.slug.as_str())
        != Some("GMAIL_NEW_MESSAGE")
    {
        return Err(
            io::Error::other("available trigger listing did not decode the fixture").into(),
        );
    }
    let active: tinyconnectors_bus::ComposioActiveTriggersResponse = proxy
        .call(
            methods::LIST_TRIGGERS,
            (ComposioListTriggersRequest { toolkit: None },),
        )
        .await?;
    if active.triggers.first().map(|trigger| trigger.id.as_str()) != Some("trigger_1") {
        return Err(io::Error::other("active trigger listing did not decode the fixture").into());
    }
    let created: tinyconnectors_bus::ComposioCreateTriggerResponse = proxy
        .call(
            methods::CREATE_TRIGGER,
            (ComposioCreateTriggerRequest {
                slug: "GMAIL_NEW_MESSAGE".into(),
                connection_id: Some("conn_1".into()),
                trigger_config: None,
            },),
        )
        .await?;
    if created.trigger_id != "trigger_new" {
        return Err(io::Error::other("trigger creation did not return the fixture id").into());
    }
    let enabled: tinyconnectors_bus::ComposioEnableTriggerResponse = proxy
        .call(
            methods::ENABLE_TRIGGER,
            (ComposioEnableTriggerRequest {
                connection_id: "conn_1".into(),
                slug: "GMAIL_NEW_MESSAGE".into(),
                trigger_config: None,
            },),
        )
        .await?;
    if enabled.trigger_id != "trigger_new" {
        return Err(io::Error::other("trigger enable did not return the fixture id").into());
    }
    let disabled: tinyconnectors_bus::ComposioDisableTriggerResponse = proxy
        .call(
            methods::DISABLE_TRIGGER,
            (ComposioDisableTriggerRequest {
                trigger_id: "trigger_new".into(),
            },),
        )
        .await?;
    if !disabled.deleted {
        return Err(io::Error::other("trigger disable was not acknowledged").into());
    }

    let deleted: tinyconnectors_bus::ComposioDeleteResponse = proxy
        .call(
            methods::DELETE_CONNECTION,
            (ComposioDeleteConnectionRequest {
                connection_id: "conn_old".into(),
            },),
        )
        .await?;
    if !deleted.deleted {
        return Err(io::Error::other("connection deletion was not acknowledged").into());
    }

    let seen = requests.lock().unwrap();
    for endpoint in [
        "/agent-integrations/composio/toolkits",
        "/agent-integrations/composio/connections",
        "/agent-integrations/composio/authorize",
        "/agent-integrations/composio/tools?",
        "/agent-integrations/composio/execute",
        "/agent-integrations/composio/github/repos?",
        "/agent-integrations/composio/triggers/available?",
        "/agent-integrations/composio/triggers",
    ] {
        if !seen.iter().any(|request| request.contains(endpoint)) {
            return Err(io::Error::other(format!("loaded module did not call {endpoint}")).into());
        }
    }
    Ok(())
}

fn mock_backend() -> Result<(String, std::sync::Arc<std::sync::Mutex<Vec<String>>>), io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            if serve_mock_request(stream, &captured).is_err() {
                break;
            }
        }
    });
    Ok((format!("http://{address}"), requests))
}

fn serve_mock_request(
    stream: TcpStream,
    captured: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) -> Result<(), io::Error> {
    let mut reader = std::io::BufReader::new(stream.try_clone()?);
    let mut request_head = String::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        request_head.push_str(&line);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;
    let request_line = request_head.lines().next().unwrap_or_default().to_string();
    captured.lock().unwrap().push(request_line.clone());
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let data = if path.ends_with("/toolkits") {
        serde_json::json!({"toolkits":["gmail"]})
    } else if path.ends_with("/connections") {
        serde_json::json!({"connections":[{"id":"conn_1","toolkit":"gmail","status":"ACTIVE"}]})
    } else if path.ends_with("/authorize") {
        serde_json::json!({"connectUrl":"https://connect.example.test/oauth","connectionId":"conn_new"})
    } else if path.contains("/tools") {
        serde_json::json!({"tools":[{"type":"function","function":{"name":"GMAIL_SEND_EMAIL"}}]})
    } else if path.ends_with("/execute") {
        serde_json::json!({"successful":true,"data":{"sent":true,"email":"tester@example.test"}})
    } else if path.contains("/github/repos") {
        serde_json::json!({"connectionId":"conn_1","repositories":[{"owner":"tinyhumans","repo":"example","fullName":"tinyhumans/example"}]})
    } else if path.contains("/triggers/available") {
        serde_json::json!({"triggers":[{"slug":"GMAIL_NEW_MESSAGE","scope":"static"}]})
    } else if path.ends_with("/triggers") && request_line.starts_with("GET ") {
        serde_json::json!({"triggers":[{"id":"trigger_1","slug":"GMAIL_NEW_MESSAGE","toolkit":"gmail","connectionId":"conn_1"}]})
    } else if path.ends_with("/triggers") {
        serde_json::json!({"triggerId":"trigger_new","status":"enabled","slug":"GMAIL_NEW_MESSAGE","connectionId":"conn_1"})
    } else if path.contains("/triggers/") {
        serde_json::json!({"deleted":true})
    } else if path.contains("/connections/") {
        serde_json::json!({"deleted":true})
    } else {
        serde_json::json!({})
    };
    let envelope = serde_json::json!({"success":true,"data":data});
    let response = envelope.to_string();
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.len(),
        response
    )?;
    stream.flush()
}

/// Require the compiled module path supplied to this verifier.
fn module_argument() -> Result<PathBuf, io::Error> {
    std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: cargo run --example verify_module -- <module-path>",
            )
        })
}

/// Prove migrated vocabulary through the loaded binary, not a linked library.
async fn verify_vocabulary(proxy: &tinybus::Proxy) -> Result<(), Box<dyn std::error::Error>> {
    verify_catalogs(proxy).await?;
    verify_action_vocabulary(proxy).await?;
    verify_identity_task_vocabulary(proxy).await?;
    Ok(())
}

async fn verify_catalogs(proxy: &tinybus::Proxy) -> Result<(), Box<dyn std::error::Error>> {
    let golden: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(
        include_str!("../src/vocabulary/catalogs/fixtures/catalog-actions.json"),
    )?;
    let reply: VocabularyResponse = proxy
        .call(
            names::methods::GET_VOCABULARY,
            (VocabularyRequest {
                toolkits: golden.keys().cloned().collect(),
            },),
        )
        .await?;
    for toolkit in reply.toolkits {
        if serde_json::to_value(&toolkit.actions)? != golden[&toolkit.toolkit] {
            return Err(
                io::Error::other(format!("catalog changed for {}", toolkit.toolkit)).into(),
            );
        }
    }
    let descriptions: std::collections::BTreeMap<String, String> = serde_json::from_str(
        include_str!("../src/vocabulary/catalogs/fixtures/descriptions.json"),
    )?;
    let described: VocabularyResponse = proxy
        .call(
            names::methods::GET_VOCABULARY,
            (VocabularyRequest {
                toolkits: descriptions.keys().cloned().collect(),
            },),
        )
        .await?;
    for toolkit in described.toolkits {
        if toolkit.description != descriptions[&toolkit.toolkit] {
            return Err(
                io::Error::other(format!("description changed for {}", toolkit.toolkit)).into(),
            );
        }
    }
    if reply.agent_ready_toolkits != golden.keys().cloned().collect::<Vec<_>>() {
        return Err(io::Error::other("catalog-ready toolkit vocabulary changed").into());
    }
    Ok(())
}

async fn verify_action_vocabulary(
    proxy: &tinybus::Proxy,
) -> Result<(), Box<dyn std::error::Error>> {
    let malformed: tinybus::Result<Vec<ActionVocabularyResponse>> = proxy.call(names::methods::EVALUATE_ACTIONS,
        (serde_json::json!([{"slug":"CUSTOM_LIST_DRAFTS","preference":{},"unknown_scope_profile":"unknown"}]),)).await;
    if malformed.is_ok() {
        return Err(io::Error::other("unknown classification profile was accepted").into());
    }
    let actions: Vec<ActionVocabularyResponse> = proxy.call(names::methods::EVALUATE_ACTIONS,
        (serde_json::json!([
            {"slug":"CUSTOM_LIST_DRAFTS", "preference":{"read":true,"write":false,"admin":false},
             "unknown_scope_profile":"substring_compatibility","compare_with":null},
            {"slug":"CUSTOM_LIST_DRAFTS", "preference":{"read":true,"write":false,"admin":false},
             "unknown_scope_profile":"leading_verb","compare_with":null},
            {"slug":"GOOGLE_CALENDAR_CREATE_EVENT", "preference":{},
             "unknown_scope_profile":"substring_compatibility","compare_with":"googlecalendar_create_event"}
        ]),)).await?;
    if actions.len() != 3
        || actions[0].visible
        || !actions[1].visible
        || actions[2].matches != Some(false)
        || actions[2].canonical_slug.as_deref() != Some("GOOGLECALENDAR_CREATE_EVENT")
    {
        return Err(io::Error::other("action alias or classifier profile changed").into());
    }
    Ok(())
}

async fn verify_identity_task_vocabulary(
    proxy: &tinybus::Proxy,
) -> Result<(), Box<dyn std::error::Error>> {
    let identities: NormalizeIdentitiesResponse = proxy.call(names::methods::NORMALIZE_IDENTITIES,
        (serde_json::json!({
            "values":[{"kind":"email","value":" Alice@Example.COM "},{"kind":"handle","value":" @Alice "},
                      {"kind":"user_id","value":"  "},{"kind":"avatar_url","value":"https://example.test/a"}],
            "connection_identifiers":[" Conn-1 "],
            "identities":[{"source":"slack","identifier":"conn-1","display_name":"Alice W",
                           "email":"alice@example.com","handle":"alicew","user_id":"U123ABC"}]
        }),)).await?;
    if identities.values.len() != 4
        || identities.values[0].value.as_deref() != Some("alice@example.com")
        || identities.values[1].value.as_deref() != Some("alice")
        || identities.values[2].value.is_some()
        || identities.values[3].matchable
        || identities.connection_identifiers != ["conn-1"]
        || identities.prompt_section
            != "## Connected Identities\n\n- Slack (conn-1): Alice W | alice@example.com | @alicew\n"
    {
        return Err(
            io::Error::other("durable identity normalization or prompt output changed").into(),
        );
    }
    for (max, expected) in [(0, 25), (3, 3)] {
        let prepared: PreparedTaskFilter = proxy
            .call(
                names::methods::PREPARE_TASK_FILTER,
                (serde_json::json!({"max":max,"extra":{"future":"kept"}}),),
            )
            .await?;
        if prepared.effective_max != expected
            || prepared.filter.extra != serde_json::json!({"future":"kept"})
        {
            return Err(io::Error::other("task envelope or default cap changed").into());
        }
    }
    Ok(())
}
