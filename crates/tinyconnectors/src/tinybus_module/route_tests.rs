use super::*;
use tinybus::{Connection, broker::Broker, transport::memory::MemoryBus};
use tinyconnectors_bus::{ComposioConfigureResponse, names};

#[tokio::test]
async fn routed_member_refuses_ordinary_delivery_before_using_credentials() {
    let (a, seen_a) = server(|_| {
        json(r#"{"connections":[{"id":"route-a","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let (b, seen_b) = server(|_| {
        json(r#"{"connections":[{"id":"route-b","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let bus = MemoryBus::new();
    Broker::new().spawn(bus.clone());
    let serving = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    serving
        .serve_at(names::OBJECT_PATH.try_into().unwrap(), unrouted_service())
        .await
        .unwrap();
    serving.request_name(names::INTERFACE).await.unwrap();
    let caller = Connection::connect(bus.connect().await.unwrap())
        .await
        .unwrap();
    let proxy_a = caller
        .proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)
        .unwrap();
    let proxy_b = caller
        .proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)
        .unwrap();
    let config_a = ComposioConfigureRequest::Proxy {
        base_url: a.clone(),
        auth_token: "token-a".into(),
        timezone: None,
        transport: None,
    };
    let config_b = ComposioConfigureRequest::Proxy {
        base_url: b,
        auth_token: "token-b".into(),
        timezone: None,
        transport: None,
    };
    proxy_a
        .call::<ComposioConfigureResponse>(names::methods::CONFIGURE, config_a)
        .await
        .unwrap();
    proxy_b
        .call::<ComposioConfigureResponse>(names::methods::CONFIGURE, config_b)
        .await
        .unwrap();
    let error = proxy_a
        .call::<tinyconnectors_bus::ComposioRoutedReadReply>(
            names::methods::ROUTED_READ,
            routed(
                a,
                "token-a",
                tinyconnectors_bus::ComposioReadOperation::ListConnections,
            ),
        )
        .await
        .expect_err("ordinary delivery must refuse credential-bearing requests");
    assert!(error.to_string().contains("requires a confidential call"));
    assert!(seen_a.lock().unwrap().is_empty());
    assert!(seen_b.lock().unwrap().is_empty());
}

fn routed(
    base: String,
    token: &str,
    operation: tinyconnectors_bus::ComposioReadOperation,
) -> tinyconnectors_bus::ComposioRoutedReadRequest {
    tinyconnectors_bus::ComposioRoutedReadRequest {
        route: tinyconnectors_bus::ComposioCallRoute::Proxy {
            base_url: base,
            auth_token: token.into(),
            timezone: None,
            transport: None,
        },
        operation,
    }
}

#[tokio::test]
async fn simultaneous_requests_keep_url_and_authorization_independent() {
    use tinyconnectors_bus::{ComposioReadOperation as Op, ComposioRoutedReadReply as Reply};
    let (a, seen_a) = server(|_| {
        json(r#"{"connections":[{"id":"route-a","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let (b, seen_b) = server(|_| {
        json(r#"{"connections":[{"id":"route-b","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let service = unrouted_service();
    let (a, b) = tokio::join!(
        service.routed_read(routed(a, "token-a", Op::ListConnections)),
        service.routed_read(routed(b, "token-b", Op::ListConnections))
    );
    let (Reply::ListConnections(a), Reply::ListConnections(b)) = (a.unwrap(), b.unwrap()) else {
        panic!("wrong reply variants");
    };
    assert_eq!(a.connections[0].id, "route-a");
    assert_eq!(b.connections[0].id, "route-b");
    let a = seen_a.lock().unwrap()[0].to_ascii_lowercase();
    let b = seen_b.lock().unwrap()[0].to_ascii_lowercase();
    assert!(a.contains("authorization: bearer token-a") && !a.contains("token-b"));
    assert!(b.contains("authorization: bearer token-b") && !b.contains("token-a"));
    assert!(
        service.client().is_err(),
        "routed calls must install no shared client"
    );
}

#[tokio::test]
async fn inflight_read_survives_other_route_and_signout_without_shared_client_read() {
    use tinyconnectors_bus::{ComposioReadOperation as Op, ComposioRoutedReadReply as Reply};
    let (reached_tx, reached) = tokio::sync::oneshot::channel();
    let reached_tx = std::sync::Mutex::new(Some(reached_tx));
    let (release, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Mutex::new(release_rx);
    let (a, seen_a) = server(move |_| {
        reached_tx.lock().unwrap().take().unwrap().send(()).unwrap();
        release_rx.lock().unwrap().recv().unwrap();
        json(r#"{"connections":[{"id":"route-a","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let (b, _) = server(|_| {
        json(r#"{"connections":[{"id":"route-b","toolkit":"gmail","status":"ACTIVE"}]}"#)
    });
    let service = Arc::new(unrouted_service());
    let pending_service = service.clone();
    let pending = tokio::spawn(async move {
        pending_service
            .routed_read(routed(a, "token-a", Op::ListConnections))
            .await
    });
    reached.await.unwrap();
    service
        .configure(ComposioConfigureRequest::Proxy {
            base_url: b.clone(),
            auth_token: "token-b".into(),
            timezone: None,
            transport: None,
        })
        .await
        .unwrap();
    let Reply::ListConnections(second) = service
        .routed_read(routed(b, "token-b", Op::ListConnections))
        .await
        .unwrap()
    else {
        panic!("wrong reply");
    };
    assert_eq!(second.connections[0].id, "route-b");
    service
        .configure(ComposioConfigureRequest::None)
        .await
        .unwrap();
    release.send(()).unwrap();
    let Reply::ListConnections(first) = pending.await.unwrap().unwrap() else {
        panic!("wrong reply");
    };
    assert_eq!(first.connections[0].id, "route-a");
    assert!(
        seen_a.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer token-a")
    );
    assert!(service.client().is_err());
}

#[tokio::test]
async fn five_proxy_reads_preserve_method_shapes_and_trigger_filters() {
    use tinyconnectors_bus::{ComposioReadOperation as Op, ComposioRoutedReadReply as Reply};
    let (base, seen) = server(|head| {
        if head.contains("triggers") {
            json(r#"{"triggers":[]}"#)
        } else if head.contains("repositories") || head.contains("github/repos") {
            json(r#"{"connectionId":"conn","repositories":[]}"#)
        } else if head.contains("toolkits") {
            json(r#"{"toolkits":["gmail"]}"#)
        } else {
            json(r#"{"connections":[]}"#)
        }
    });
    let service = unrouted_service();
    assert!(matches!(
        service
            .routed_read(routed(base.clone(), "token", Op::ListToolkits))
            .await
            .unwrap(),
        Reply::ListToolkits(_)
    ));
    assert!(matches!(
        service
            .routed_read(routed(base.clone(), "token", Op::ListConnections))
            .await
            .unwrap(),
        Reply::ListConnections(_)
    ));
    assert!(matches!(
        service
            .routed_read(routed(
                base.clone(),
                "token",
                Op::ListGithubRepos(tinyconnectors_bus::ComposioListGithubReposRequest {
                    connection_id: Some("conn".into())
                })
            ))
            .await
            .unwrap(),
        Reply::ListGithubRepos(_)
    ));
    assert!(matches!(
        service
            .routed_read(routed(
                base.clone(),
                "token",
                Op::ListAvailableTriggers(
                    tinyconnectors_bus::ComposioListAvailableTriggersRequest {
                        toolkit: "github".into(),
                        connection_id: Some("conn".into())
                    }
                )
            ))
            .await
            .unwrap(),
        Reply::ListAvailableTriggers(_)
    ));
    assert!(matches!(
        service
            .routed_read(routed(
                base,
                "token",
                Op::ListTriggers(tinyconnectors_bus::ComposioListTriggersRequest {
                    toolkit: Some("github".into())
                })
            ))
            .await
            .unwrap(),
        Reply::ListTriggers(_)
    ));
    let heads = seen.lock().unwrap();
    assert_eq!(heads.len(), 5);
    assert!(heads[3].contains("toolkit=github") && heads[3].contains("connectionId=conn"));
    assert!(heads[4].contains("toolkit=github"));
}

#[tokio::test]
async fn invalid_routes_and_backend_refusals_do_not_replace_legacy_configuration() {
    use tinyconnectors_bus::ComposioReadOperation as Op;
    let service = unrouted_service();
    for request in [
        routed("https://example.com".into(), " ", Op::ListConnections),
        routed(String::new(), "secret", Op::ListConnections),
        routed(
            "http://public.example".into(),
            "secret",
            Op::ListConnections,
        ),
    ] {
        let error = service.routed_read(request).await.unwrap_err().to_string();
        assert!(!error.contains("secret"));
        assert!(service.client().is_err());
    }
    let invalid_network = tinyconnectors_bus::ComposioRoutedReadRequest {
        route: tinyconnectors_bus::ComposioCallRoute::Proxy {
            base_url: "https://example.com".into(),
            auth_token: "secret".into(),
            timezone: None,
            transport: Some(tinyconnectors_bus::ComposioTransportConfig {
                proxy_url: Some("bad-scheme://user:private-password@proxy".into()),
                ..Default::default()
            }),
        },
        operation: Op::ListConnections,
    };
    let error = service
        .routed_read(invalid_network)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("network") && !error.contains("private-password"));
    let (base, _) = server(|_| (401, Vec::new(), r#"{"error":"denied"}"#.into()));
    let error = service
        .routed_read(routed(base, "secret", Op::ListConnections))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("401") && !error.contains("secret"));
}

#[tokio::test]
async fn direct_reads_use_request_key_and_refuse_proxy_only_trigger_members() {
    use tinyconnectors_bus::{
        ComposioCallRoute, ComposioReadOperation as Op, ComposioRoutedReadReply as Reply,
        ComposioRoutedReadRequest,
    };
    let (base, seen) =
        server(|_| json(r#"{"items":[{"id":"direct-a","toolkit":"gmail","status":"ACTIVE"}]}"#));
    let service = unrouted_service();
    let request = |operation| ComposioRoutedReadRequest {
        route: ComposioCallRoute::Direct {
            credential: credential(&base),
        },
        operation,
    };
    let Reply::ListConnections(reply) = service
        .routed_read(request(Op::ListConnections))
        .await
        .unwrap()
    else {
        panic!("wrong reply");
    };
    assert_eq!(reply.connections[0].id, "direct-a");
    assert!(
        seen.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("x-api-key: ck_member")
    );
    let error = service
        .routed_read(request(Op::ListTriggers(
            tinyconnectors_bus::ComposioListTriggersRequest { toolkit: None },
        )))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("direct") && !error.contains("ck_member"));
    let empty = ComposioRoutedReadRequest {
        route: ComposioCallRoute::Direct {
            credential: ComposioDirectCredential {
                api_key: " ".into(),
                ..credential(&base)
            },
        },
        operation: Op::ListConnections,
    };
    assert!(service.routed_read(empty).await.is_err());
    let empty_url = ComposioRoutedReadRequest {
        route: ComposioCallRoute::Direct {
            credential: ComposioDirectCredential {
                base_url: Some(" ".into()),
                ..credential(&base)
            },
        },
        operation: Op::ListConnections,
    };
    assert!(service.routed_read(empty_url).await.is_err());

    assert!(service.client().is_err());
}

#[tokio::test]
async fn invalid_endpoint_refusal_does_not_echo_embedded_route_credentials() {
    let error = unrouted_service()
        .routed_read(routed(
            "https://user:private-password@private.example".into(),
            "secret-bearer",
            tinyconnectors_bus::ComposioReadOperation::ListConnections,
        ))
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("embedded credentials"));
    for secret in ["private-password", "secret-bearer", "private.example"] {
        assert!(
            !error.contains(secret),
            "credential-bearing endpoint must not enter a reply"
        );
    }
}
