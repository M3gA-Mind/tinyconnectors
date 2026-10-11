//! Loads a built module through the real `TinyBus` dynamic loader.

use std::io;
use std::path::PathBuf;
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
    let bus = MemoryBus::new();
    let broker = Broker::new();
    let broker_task = broker.spawn(bus.clone());
    let module_host = ModuleHost::new(broker);
    // All verifier operations are local and must work without credentials.
    let info = module_host.load_file_with_config(&module, serde_json::json!({}))?;

    if info.name != env!("CARGO_PKG_NAME") {
        return Err(io::Error::other(format!(
            "loaded module `{}` instead of `{}`",
            info.name,
            env!("CARGO_PKG_NAME")
        ))
        .into());
    }

    verify_served_surface(&bus, &info).await?;
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
    if reply.agent_ready_toolkits.len() != 31 {
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
