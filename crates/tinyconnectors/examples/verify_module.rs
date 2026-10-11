//! Loads a built module through the real `TinyBus` dynamic loader.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use tinybus::Connection;
use tinybus::broker::Broker;
use tinybus::module::{ModuleHost, ModuleInfo};
use tinybus::transport::memory::MemoryBus;
use tinyconnectors::names;

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
    // The module refuses to load without a route and the credential that route
    // needs, which is the point: it never invents its own. These are
    // placeholders — the verifier checks the served surface, not a live call.
    let info = module_host
        .load_file_with_config(
            &module,
            serde_json::json!({
                "route": "proxy",
                "base_url": "https://verify.invalid",
                "auth_token": "verify-only-not-a-credential",
            }),
        )
        .map_err(|error| io::Error::other(format!("classic initialization: {error}")))?;

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
    Ok(())
}

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
