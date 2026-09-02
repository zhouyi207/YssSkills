use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use skill_registry::{
    parse_git_source, parse_leaderboard_html, parse_search_response, parse_source_reference,
    resolve_tree_branch_path, Leaderboard, QueryValidationError, RegistryError, RetryAfter,
    SkillsShClient, SourceKind, SourceParseError, TransportKind, TransportOperation,
    DEFAULT_MAX_RESPONSE_BYTES, MIN_QUERY_LENGTH,
};

struct HttpFixture {
    base_url: String,
    request: Arc<Mutex<Vec<u8>>>,
    handle: Option<JoinHandle<()>>,
}

impl HttpFixture {
    fn finish(mut self) -> String {
        let handle = self.handle.take().expect("fixture thread should exist");
        handle.join().expect("fixture server should finish");
        let request = self
            .request
            .lock()
            .expect("request lock should not be poisoned");
        String::from_utf8_lossy(&request).to_string()
    }
}

fn fixture(status: u16, reason: &str, body: Vec<u8>, extra_headers: &str) -> HttpFixture {
    fixture_with_transfer_encoding(status, reason, body, extra_headers, false)
}

fn chunked_fixture(status: u16, reason: &str, body: Vec<u8>, extra_headers: &str) -> HttpFixture {
    fixture_with_transfer_encoding(status, reason, body, extra_headers, true)
}

fn truncated_fixture(body: Vec<u8>, declared_length: usize) -> HttpFixture {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let address = listener.local_addr().expect("fixture address should exist");
    let request = Arc::new(Mutex::new(Vec::new()));
    let request_for_thread = Arc::clone(&request);

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("fixture should accept one request");
        read_request(&mut stream, &request_for_thread);
        let response = format!(
            "HTTP/1.1 200 OK\\r\\nContent-Length: {declared_length}\\r\\nConnection: close\\r\\n\\r\\n"
        );
        stream
            .write_all(response.as_bytes())
            .expect("fixture headers should be written");
        stream
            .write_all(&body)
            .expect("fixture body should be written");
    });

    HttpFixture {
        base_url: format!("http://{address}/"),
        request,
        handle: Some(handle),
    }
}

fn fixture_with_transfer_encoding(
    status: u16,
    reason: &str,
    body: Vec<u8>,
    extra_headers: &str,
    chunked: bool,
) -> HttpFixture {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let address = listener.local_addr().expect("fixture address should exist");
    let request = Arc::new(Mutex::new(Vec::new()));
    let request_for_thread = Arc::clone(&request);
    let reason = reason.to_owned();
    let extra_headers = extra_headers.to_owned();

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("fixture should accept one request");
        read_request(&mut stream, &request_for_thread);
        let response = if chunked {
            format!(
                "HTTP/1.1 {status} {reason}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n{extra_headers}\r\n"
            )
        } else {
            format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n",
                body.len()
            )
        };
        stream
            .write_all(response.as_bytes())
            .expect("fixture headers should be written");
        if chunked {
            for chunk in body.chunks(3) {
                let size = format!("{:X}\r\n", chunk.len());
                if stream.write_all(size.as_bytes()).is_err()
                    || stream.write_all(chunk).is_err()
                    || stream.write_all(b"\r\n").is_err()
                {
                    return;
                }
            }
            // The client may close after reaching its configured response limit.
            drop(stream.write_all(b"0\r\n\r\n"));
        } else {
            stream
                .write_all(&body)
                .expect("fixture body should be written");
        }
    });

    HttpFixture {
        base_url: format!("http://{address}/"),
        request,
        handle: Some(handle),
    }
}

fn read_request(stream: &mut TcpStream, request: &Arc<Mutex<Vec<u8>>>) {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .expect("fixture request should be readable");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if bytes.len() > 64 * 1024 {
            break;
        }
    }
    *request.lock().expect("request lock should not be poisoned") = bytes;
}

#[test]
fn client_builds_encoded_search_url_and_uses_expected_leaderboard_paths() {
    let fixture = fixture(
        200,
        "OK",
        br#"{"skills":[{"source":"acme/tools","skillId":"search","name":"Search","installs":3}]}"#
            .to_vec(),
        "",
    );
    let client = SkillsShClient::builder()
        .base_url(&fixture.base_url)
        .build()
        .expect("fixture client should build");

    let url = client
        .search_url(" rust tools/中文 ", 12)
        .expect("query should be valid");
    assert_eq!(url.path(), "/api/search");
    assert_eq!(
        url.query_pairs()
            .find(|(key, _)| key == "q")
            .map(|(_, value)| value.into_owned()),
        Some("rust tools/中文".to_owned())
    );
    assert_eq!(
        url.query_pairs()
            .find(|(key, _)| key == "limit")
            .map(|(_, value)| value.into_owned()),
        Some("12".to_owned())
    );

    assert_eq!(
        client.leaderboard_url(Leaderboard::AllTime).unwrap().path(),
        "/"
    );
    assert_eq!(
        client
            .leaderboard_url(Leaderboard::Trending)
            .unwrap()
            .path(),
        "/trending"
    );
    assert_eq!(
        client.leaderboard_url(Leaderboard::Hot).unwrap().path(),
        "/hot"
    );

    let result = client
        .search("rust tools/中文", 12)
        .expect("search should parse");
    assert_eq!(result.skills.len(), 1);
    let request = fixture.finish();
    assert!(request.starts_with("GET /api/search?"));
    assert!(request.contains("limit=12"));
    assert!(request.contains("q=rust+tools%2F%E4%B8%AD%E6%96%87"));
}

#[test]
fn client_fetches_leaderboard_from_local_fixture() {
    let fixture = fixture(
        200,
        "OK",
        br#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"initialSkills":[{"source":"acme/tools","skillId":"leader","name":"Leader","installs":5}]}}}</script>"#.to_vec(),
        "",
    );
    let client = SkillsShClient::with_base_url(&fixture.base_url).unwrap();

    let result = client
        .leaderboard(Leaderboard::Trending)
        .expect("leaderboard should parse");
    assert_eq!(result.leaderboard, Leaderboard::Trending);
    assert_eq!(result.skills.len(), 1);
    assert_eq!(result.skills[0].name, "Leader");

    let request = fixture.finish();
    assert!(request.starts_with("GET /trending HTTP/1.1\r\n"));
}

#[test]
fn client_fetches_redacted_real_rsc_record_stream_from_local_fixture() {
    let fixture = fixture(
        200,
        "OK",
        br#"<script>self.__next_f.push([1,"1:\"$Sreact.fragment\"\n2:I[\"$\",\"$L57\",null]\n49:\"$undefined\"\n50:[\"$\",\"$L57\",null,{\"initialSkills\":[{\"source\":\"redacted-owner/redacted-skills\",\"skillId\":\"redacted-skill\",\"name\":\"Redacted Skill\",\"installs\":22057}]}]\n51:I[\"$\",\"$L58\",null]"])</script>"#.to_vec(),
        "",
    );
    let client = SkillsShClient::with_base_url(&fixture.base_url).unwrap();

    let result = client
        .leaderboard(Leaderboard::Trending)
        .expect("record-stream leaderboard should parse");
    assert_eq!(result.leaderboard, Leaderboard::Trending);
    assert_eq!(result.skills.len(), 1);
    assert_eq!(
        result.skills[0].id.to_string(),
        "redacted-owner/redacted-skills/redacted-skill"
    );
    assert_eq!(result.skills[0].installs, 22057);

    let request = fixture.finish();
    assert!(request.starts_with("GET /trending HTTP/1.1\r\n"));
}

#[test]
fn client_fetches_hex_record_id_from_real_rsc_shape() {
    let fixture = fixture(
        200,
        "OK",
        br#"<script>self.__next_f.push([1,"4e:[\"$\",\"$L57\",null,{\"initialSkills\":[{\"source\":\"vercel-labs/skills\",\"skillId\":\"find-skills\",\"name\":\"Find Skills\",\"installs\":12345}]}]"])</script>"#.to_vec(),
        "",
    );
    let client = SkillsShClient::with_base_url(&fixture.base_url).unwrap();

    let result = client
        .leaderboard(Leaderboard::AllTime)
        .expect("hex record id should parse");
    assert_eq!(result.skills.len(), 1);
    assert_eq!(
        result.skills[0].id.to_string(),
        "vercel-labs/skills/find-skills"
    );
    assert_eq!(result.skills[0].installs, 12345);

    let request = fixture.finish();
    assert!(request.starts_with("GET / HTTP/1.1\r\n"));
}

#[test]
fn client_maps_authentication_rate_limit_and_other_http_statuses() {
    let auth_fixture = fixture(
        401,
        "Unauthorized",
        b"not used".to_vec(),
        "Retry-After: Wed, 21 Oct 2015 07:28:00 GMT\r\n",
    );
    let auth_client = SkillsShClient::with_base_url(&auth_fixture.base_url).unwrap();
    assert!(matches!(
        auth_client.search("query", 1),
        Err(RegistryError::AuthenticationRequired {
            status: 401,
            retry_after: Some(RetryAfter::At(_))
        })
    ));
    auth_fixture.finish();

    let rate_fixture = fixture(
        429,
        "Too Many Requests",
        b"not used".to_vec(),
        "Retry-After: 17\r\n",
    );
    let rate_client = SkillsShClient::with_base_url(&rate_fixture.base_url).unwrap();
    assert!(matches!(
        rate_client.search("query", 1),
        Err(RegistryError::RateLimited {
            status: 429,
            retry_after: Some(RetryAfter::Delay(delay))
        }) if delay == Duration::from_secs(17)
    ));
    rate_fixture.finish();

    let server_fixture = fixture(
        503,
        "Unavailable",
        b"not used".to_vec(),
        "Retry-After: Wed, 21 Oct 2015 07:28:00 GMT\r\n",
    );
    let server_client = SkillsShClient::with_base_url(&server_fixture.base_url).unwrap();
    assert!(matches!(
        server_client.search("query", 1),
        Err(RegistryError::HttpStatus {
            status: 503,
            retry_after: Some(RetryAfter::At(_))
        })
    ));
    server_fixture.finish();
}

#[test]
fn client_rejects_oversized_and_invalid_responses() {
    let too_large_limit = SkillsShClient::builder()
        .max_response_bytes(DEFAULT_MAX_RESPONSE_BYTES + 1)
        .build();
    assert!(matches!(
        too_large_limit,
        Err(RegistryError::ResponseLimitTooLarge {
            requested,
            maximum
        }) if requested == DEFAULT_MAX_RESPONSE_BYTES + 1
            && maximum == DEFAULT_MAX_RESPONSE_BYTES
    ));
    assert!(SkillsShClient::builder()
        .max_response_bytes(DEFAULT_MAX_RESPONSE_BYTES)
        .build()
        .is_ok());

    let large_fixture = fixture(200, "OK", b"123456789".to_vec(), "");
    let large_client = SkillsShClient::builder()
        .base_url(&large_fixture.base_url)
        .max_response_bytes(4)
        .build()
        .unwrap();
    assert!(matches!(
        large_client.search("query", 1),
        Err(RegistryError::ResponseTooLarge {
            limit: 4,
            observed: Some(9)
        })
    ));
    large_fixture.finish();

    let chunked_fixture = chunked_fixture(200, "OK", b"123456789".to_vec(), "");
    let chunked_client = SkillsShClient::builder()
        .base_url(&chunked_fixture.base_url)
        .max_response_bytes(4)
        .build()
        .unwrap();
    assert!(matches!(
        chunked_client.search("query", 1),
        Err(RegistryError::ResponseTooLarge { limit: 4, .. })
    ));
    chunked_fixture.finish();

    let malformed_fixture = fixture(200, "OK", b"{broken".to_vec(), "");
    let malformed_client = SkillsShClient::with_base_url(&malformed_fixture.base_url).unwrap();
    assert!(matches!(
        malformed_client.search("query", 1),
        Err(RegistryError::InvalidResponse {
            kind: skill_registry::ResponseKind::Search,
            ..
        })
    ));
    malformed_fixture.finish();

    let missing_fixture = fixture(200, "OK", br#"{"data":[]}"#.to_vec(), "");
    let missing_client = SkillsShClient::with_base_url(&missing_fixture.base_url).unwrap();
    assert!(matches!(
        missing_client.search("query", 1),
        Err(RegistryError::MissingResponseField {
            kind: skill_registry::ResponseKind::Search,
            ..
        })
    ));
    missing_fixture.finish();
}

#[test]
fn client_classifies_transport_errors_without_exposing_details() {
    let fixture = truncated_fixture(b"{}".to_vec(), 3);
    let client = SkillsShClient::with_base_url(&fixture.base_url).unwrap();
    let error = client
        .search("query", 1)
        .expect_err("truncated body should fail");

    assert_eq!(
        error.to_string(),
        "registry transport failed during search request (request)"
    );
    assert!(matches!(
        error,
        RegistryError::Transport {
            operation: TransportOperation::SearchRequest,
            kind: TransportKind::Request
        }
    ));
    fixture.finish();
}

#[test]
fn search_parser_accepts_envelope_and_legacy_array_and_merges_duplicates() {
    let envelope = br#"{
        "skills": [
            {
                "source": "registry.example/tools",
                "skillId": "writer",
                "name": "",
                "installs": 0,
                "source_kind": "well-known",
                "install_url": "https://registry.example/tools/writer",
                "is_official": true,
                "skills_sh_url": "https://skills.sh/registry.example/tools/writer"
            },
            {
                "source": "registry.example/tools",
                "skill_id": "writer",
                "name": "Writer",
                "installs": 8
            }
        ]
    }"#;
    let parsed = parse_search_response(envelope).expect("envelope should parse");
    assert_eq!(parsed.skills.len(), 1);
    let skill = &parsed.skills[0];
    assert_eq!(skill.id.source, "registry.example/tools");
    assert_eq!(skill.id.skill_id, "writer");
    assert_eq!(skill.name, "Writer");
    assert_eq!(skill.installs, 8);
    assert_eq!(skill.source_kind, Some(SourceKind::WellKnown));
    assert_eq!(
        skill.install_url.as_deref(),
        Some("https://registry.example/tools/writer")
    );
    assert_eq!(skill.is_official, Some(true));

    let legacy = br#"[
        {"source":"acme/skills","skill_id":"one"},
        {"source":"acme/skills","skillId":"two","name":"Two","installs":2}
    ]"#;
    let legacy_result = parse_search_response(legacy).expect("top-level array should parse");
    assert_eq!(legacy_result.skills.len(), 2);
    assert_eq!(legacy_result.skills[0].name, "one");
    assert_eq!(legacy_result.skills[0].installs, 0);
}

#[test]
fn search_parser_rejects_error_envelopes_even_with_skills() {
    let assert_invalid = |body: &[u8]| {
        assert!(matches!(
            parse_search_response(body),
            Err(RegistryError::InvalidResponse {
                kind: skill_registry::ResponseKind::Search,
                ..
            })
        ));
    };

    assert_invalid(br#"{"error":"upstream search failed","skills":[]}"#);
    assert_invalid(
        br#"{"errors":[{"code":"upstream_failure"}],"skills":[{"source":"acme/tools","skillId":"writer","name":"Writer","installs":1}]}"#,
    );

    let invalid_values: &[&[u8]] = &[
        br#"{"error":false,"skills":[]}"#,
        br#"{"errors":0,"skills":[]}"#,
        br#"{"error":{},"skills":[]}"#,
        br#"{"errors":{"code":"upstream_failure"},"skills":[]}"#,
    ];
    for body in invalid_values {
        assert_invalid(body);
    }

    let assert_empty_success = |body: &[u8]| {
        let parsed = parse_search_response(body).expect("empty error sentinel should succeed");
        assert!(parsed.skills.is_empty());
    };
    assert_empty_success(br#"{"error":null,"skills":[]}"#);
    assert_empty_success(br#"{"error":"","skills":[]}"#);
    assert_empty_success(br#"{"errors":" ","skills":[]}"#);
    assert_empty_success(br#"{"errors":[],"skills":[]}"#);

    let empty_success =
        parse_search_response(br#"{"skills":[]}"#).expect("empty skills should succeed");
    assert!(empty_success.skills.is_empty());
}

#[test]
fn leaderboard_parser_accepts_next_data_and_current_rsc_objects() {
    let next = r#"
        <script id="__NEXT_DATA__" type="application/json">
          {"props":{"pageProps":{"initialSkills":[
            {"source":"antfu/skills","skillId":"vite","name":"vite","installs":152,"source_kind":"github","is_official":false},
            {"source":"antfu/skills","skill_id":"vite","name":"Vite","installs":152}
          ]}}}
        </script>
    "#;
    let next_skills = parse_leaderboard_html(next).expect("Next data should parse");
    assert_eq!(next_skills.len(), 1);
    assert_eq!(next_skills[0].id.to_string(), "antfu/skills/vite");
    assert_eq!(next_skills[0].name, "Vite");
    assert_eq!(next_skills[0].source_kind, Some(SourceKind::GitHub));
    assert_eq!(next_skills[0].is_official, Some(false));

    let rsc = r#"
      <script>self.__next_f.push([1,"{\"props\":{\"pageProps\":{\"initialSkills\":[{\"source\":\"anthropics/skills\",\"skillId\":\"template-skill\",\"name\":\"template-skill\",\"installs\":238},{\"source\":\"vercel/ai\",\"skill_id\":\"ai-sdk\"}]}}}"])</script>
    "#;
    let rsc_skills = parse_leaderboard_html(rsc).expect("RSC objects should parse");
    assert_eq!(rsc_skills.len(), 2);
    assert_eq!(
        rsc_skills[0].id.to_string(),
        "anthropics/skills/template-skill"
    );
    assert_eq!(rsc_skills[1].name, "ai-sdk");
    assert_eq!(rsc_skills[1].installs, 0);
}

#[test]
fn leaderboard_parser_accepts_next_router_undefined_error_sentinels() {
    let rsc = r#"
      <script>self.__next_f.push([1,"1:[\"$\",\"$L1\",null,{\"error\":\"$undefined\",\"errorStyles\":\"$undefined\"}]\n2:[\"$\",\"$L2\",null,{\"initialSkills\":[{\"source\":\"vercel-labs/skills\",\"skillId\":\"find-skills\",\"name\":\"find-skills\",\"installs\":42}]}]"])</script>
    "#;

    let skills = parse_leaderboard_html(rsc).expect("Next router sentinels should not be errors");

    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id.to_string(), "vercel-labs/skills/find-skills");
}

#[test]
fn leaderboard_parser_rejects_unrelated_next_arrays_but_accepts_explicit_empty_payload() {
    let invalid = r#"
        <script id="__NEXT_DATA__" type="application/json">
          {"props":{"pageProps":{"telemetry":[]}}}
        </script>
    "#;
    assert!(matches!(
        parse_leaderboard_html(invalid),
        Err(RegistryError::MissingResponseField {
            kind: skill_registry::ResponseKind::Leaderboard,
            ..
        })
    ));

    let valid_empty = r#"
        <script id="__NEXT_DATA__" type="application/json">
          {"props":{"pageProps":{"initialSkills":[]},"telemetry":[]}}
        </script>
    "#;
    assert_eq!(
        parse_leaderboard_html(valid_empty).unwrap(),
        Vec::<skill_registry::RemoteSkillSummary>::new()
    );

    let rsc_empty = r#"
      <script>self.__next_f.push([1,"{\"props\":{\"pageProps\":{\"initialSkills\":[]}}}"])</script>
    "#;
    assert_eq!(
        parse_leaderboard_html(rsc_empty).unwrap(),
        Vec::<skill_registry::RemoteSkillSummary>::new()
    );

    let invalid_container = r#"
      <script id="__NEXT_DATA__" type="application/json">
        {"props":{"pageProps":{"initialSkills":false}}}
      </script>
    "#;
    assert!(matches!(
        parse_leaderboard_html(invalid_container),
        Err(RegistryError::InvalidResponse {
            kind: skill_registry::ResponseKind::Leaderboard,
            ..
        })
    ));

    let unrelated_rsc = r#"
      <script>self.__next_f.push([1,"{\"telemetry\":{\"source\":\"telemetry\",\"id\":\"event\"}}"])</script>
    "#;
    assert!(parse_leaderboard_html(unrelated_rsc).is_err());
}

#[test]
fn leaderboard_parser_requires_rsc_marker_to_be_javascript_code() {
    let marker = r#"self.__next_f.push([1,"{\"props\":{\"pageProps\":{\"initialSkills\":[]}}}"])"#;
    let in_string = format!(r#"<script>const marker = '{marker}';</script>"#);
    let in_block_comment = format!(r#"<script>/* {marker} */</script>"#);
    let in_line_comment = format!("<script>// {marker}\n</script>");
    let in_regex = format!(r#"<script>const marker = /{marker}/;</script>"#);
    let in_close_comment = format!(r#"<script>--> {marker}</script>"#);
    let in_regex_after_block = format!(r#"<script>{{}}/{marker}/</script>"#);
    let in_default_regex = format!(r#"<script>export default /{marker}/;</script>"#);
    let in_unicode_identifier = format!("<script>é{marker}</script>");

    for html in [
        in_string,
        in_block_comment,
        in_line_comment,
        in_regex,
        in_close_comment,
        in_regex_after_block,
        in_default_regex,
        in_unicode_identifier,
    ] {
        assert!(
            parse_leaderboard_html(&html).is_err(),
            "marker in non-code JavaScript must not be treated as an RSC frame"
        );
    }
}

#[test]
fn leaderboard_parser_accepts_only_real_next_data_script_tags() {
    let payload =
        r#"{"props":{"pageProps":{"initialSkills":[{"source":"acme/tools","skillId":"writer"}]}}}"#;
    let invalid_pages = [
        format!(r#"<!-- <script id="__NEXT_DATA__">{payload}</script> -->"#),
        format!(r#"<div data-marker='id="__NEXT_DATA__"'>{payload}</div>"#),
        format!(r#"<script data-marker='id="__NEXT_DATA__"'>{payload}</script>"#),
        format!(r#"<textarea><script id="__NEXT_DATA__">{payload}</script></textarea>"#),
        format!(r#"<style><script id="__NEXT_DATA__">{payload}</script></style>"#),
        format!(r#"<title><script id="__NEXT_DATA__">{payload}</script></title>"#),
        format!(r#"<noscript><script id="__NEXT_DATA__">{payload}</script></noscript>"#),
    ];

    for html in invalid_pages {
        assert!(matches!(
            parse_leaderboard_html(&html),
            Err(RegistryError::MissingResponseField {
                kind: skill_registry::ResponseKind::Leaderboard,
                ..
            })
        ));
    }

    let valid = format!(r#"<script data-marker="ignored" id="__NEXT_DATA__">{payload}</script>"#);
    let skills = parse_leaderboard_html(&valid).expect("real Next data tag should parse");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].id.to_string(), "acme/tools/writer");
}

#[test]
fn leaderboard_parser_rejects_deeply_nested_rsc_strings() {
    let mut payload = r#"{"props":{"pageProps":{"initialSkills":[]}}}"#.to_owned();
    for _ in 0..40 {
        let mut nested = String::with_capacity(payload.len() + 2);
        nested.push('"');
        for byte in payload.bytes() {
            match byte {
                b'"' => nested.push_str(r"\u0022"),
                b'\\' => nested.push_str(r"\u005c"),
                _ => nested.push(byte as char),
            }
        }
        nested.push('"');
        payload = nested;
    }
    let html = format!(r#"<script>self.__next_f.push([1,{payload}])</script>"#);

    assert!(matches!(
        parse_leaderboard_html(&html),
        Err(RegistryError::InvalidResponse {
            kind: skill_registry::ResponseKind::Leaderboard,
            ..
        })
    ));
}

#[test]
fn leaderboard_parser_preserves_explicit_kind_without_guessing_github() {
    let html = r#"
      <script>
        self.__next_f.push([1,"{\"props\":{\"pageProps\":{\"initialSkills\":[{\"source\":\"official/skills\",\"skillId\":\"calendar\",\"name\":\"Calendar\",\"installs\":4,\"source_kind\":\"well-known\",\"install_url\":\"https://official.example/calendar\",\"url\":\"https://skills.sh/official/skills/calendar\"}]}}}"])
      </script>
    "#;
    let skills = parse_leaderboard_html(html).expect("embedded object should parse");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].source_kind, Some(SourceKind::WellKnown));
    assert_eq!(
        skills[0].install_url.as_deref(),
        Some("https://official.example/calendar")
    );
    assert_eq!(
        skills[0].skills_sh_url.as_deref(),
        Some("https://skills.sh/official/skills/calendar")
    );
}

#[test]
fn skills_sh_url_uses_registry_identity_when_no_valid_url_is_available() {
    let identity_only = br#"[
        {"source":"vercel-labs/skills","skillId":"find-skills"}
    ]"#;
    let parsed = parse_search_response(identity_only).unwrap();
    assert_eq!(
        parsed.skills[0].skills_sh_url.as_deref(),
        Some("https://skills.sh/vercel-labs/skills/find-skills")
    );

    let invalid_generic_url = br#"[
        {"source":"acme/tools","skillId":"evil","url":"https://skills.sh.evil/redirect"}
    ]"#;
    let parsed = parse_search_response(invalid_generic_url).unwrap();
    assert_eq!(
        parsed.skills[0].skills_sh_url.as_deref(),
        Some("https://skills.sh/acme/tools/evil")
    );
}

#[test]
fn skills_sh_url_requires_exact_https_host() {
    let fallback_malicious = br#"[
        {"source":"acme/tools","skillId":"evil","url":"https://skills.sh.evil/redirect"}
    ]"#;
    let parsed = parse_search_response(fallback_malicious).unwrap();
    assert_eq!(
        parsed.skills[0].skills_sh_url.as_deref(),
        Some("https://skills.sh/acme/tools/evil")
    );

    let explicit_malicious = br#"[
        {"source":"acme/tools","skillId":"evil","skills_sh_url":"https://skills.sh.evil/redirect"}
    ]"#;
    assert!(matches!(
        parse_search_response(explicit_malicious),
        Err(RegistryError::InvalidResponse {
            kind: skill_registry::ResponseKind::Search,
            ..
        })
    ));

    let explicit_redirect = br#"[
        {"source":"acme/tools","skillId":"evil","skills_sh_url":"https://skills.sh@evil.example/redirect"}
    ]"#;
    assert!(matches!(
        parse_search_response(explicit_redirect),
        Err(RegistryError::InvalidResponse {
            kind: skill_registry::ResponseKind::Search,
            ..
        })
    ));
}

#[test]
fn leaderboard_parser_rejects_unscoped_objects_and_arrays() {
    let embedded_object = r#"
        <script>const telemetry = {"source":"telemetry","id":"event"};</script>
    "#;
    assert!(parse_leaderboard_html(embedded_object).is_err());

    let embedded_array = r#"
        <script>const telemetry = {"items":[{"source":"telemetry","id":"event"}]};</script>
    "#;
    assert!(parse_leaderboard_html(embedded_array).is_err());

    let root_next_array = r#"
        <script id="__NEXT_DATA__" type="application/json">
          {"skills":[{"source":"telemetry","skillId":"event"}]}
        </script>
    "#;
    assert!(parse_leaderboard_html(root_next_array).is_err());
}

#[test]
fn source_parser_handles_shorthand_tree_urls_and_known_slash_branches() {
    let shorthand = parse_git_source("owner/repo.git").unwrap();
    assert_eq!(shorthand.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(shorthand.branch, None);
    assert_eq!(shorthand.subpath, None);

    let tree =
        parse_git_source("https://github.com/owner/repo.git/tree/main/tools/my-skill").unwrap();
    assert_eq!(tree.clone_url, "https://github.com/owner/repo.git");
    assert_eq!(tree.branch.as_deref(), Some("main"));
    assert_eq!(tree.subpath.as_deref(), Some("tools/my-skill"));

    let known = vec![
        "main".to_owned(),
        "feature".to_owned(),
        "feature/x".to_owned(),
    ];
    let slash_branch = skill_registry::parse_git_source_with_branches(
        "https://github.com/owner/repo/tree/feature/x/skills/foo",
        &known,
    )
    .unwrap();
    assert_eq!(slash_branch.branch.as_deref(), Some("feature/x"));
    assert_eq!(slash_branch.subpath.as_deref(), Some("skills/foo"));

    let (branch, subpath) = resolve_tree_branch_path("feature/x/skills/foo", &[]).unwrap();
    assert_eq!(branch, "feature");
    assert_eq!(subpath.as_deref(), Some("x/skills/foo"));
}

#[test]
fn source_parser_preserves_non_github_git_urls_and_rejects_unsafe_inputs() {
    let https = parse_git_source("https://gitlab.example/acme/skills.git").unwrap();
    assert_eq!(https.clone_url, "https://gitlab.example/acme/skills.git");
    assert_eq!(https.branch, None);

    let non_github_tree =
        parse_git_source("https://gitlab.example/acme/skills/tree/main/tools").unwrap();
    assert_eq!(
        non_github_tree.clone_url,
        "https://gitlab.example/acme/skills/tree/main/tools"
    );
    assert_eq!(non_github_tree.branch, None);

    let ssh = parse_git_source("git@github.com:owner/repo.git").unwrap();
    assert_eq!(ssh.clone_url, "git@github.com:owner/repo.git");
    assert!(matches!(
        parse_git_source("git@github.com:/owner/repo.git"),
        Err(SourceParseError::InvalidUrl)
    ));

    for input in [
        "",
        "file:///tmp/repo",
        "C:/repo",
        "C:repo/foo",
        "../repo",
        "\\\\server\\share\\repo",
        "https://github.com/owner/repo/tree/main/../outside",
        "https://github.com/owner/repo/tree//skills",
        "https://github.com/owner/repo/tree/main\\..\\outside",
        "https://gitlab.example/acme\\..\\secret.git",
        "https://github.com/owner/repo%2e%2e/tree/main",
        "ssh://git@gitlab.example/acme/%2e%2e/secret.git",
        "https://github.com/owner/repo?ref=main",
        "https://github.com/owner/repo#main",
        "owner/repo/extra",
        "owner/repo\n",
    ] {
        assert!(
            parse_git_source(input).is_err(),
            "unsafe input should fail: {input:?}"
        );
    }

    assert!(matches!(
        parse_git_source("https://github.com/owner/repo/tree/main/../outside"),
        Err(SourceParseError::InvalidSubpath { .. })
    ));
}

#[test]
fn source_parser_rejects_http_and_credentials_but_keeps_ssh_username() {
    assert!(matches!(
        parse_git_source("http://gitlab.example/acme/skills.git"),
        Err(SourceParseError::UnsupportedScheme { scheme }) if scheme == "http"
    ));

    for input in [
        "https://git:password@gitlab.example/acme/skills.git",
        "https://git@gitlab.example/acme/skills.git",
        "ssh://git:password@gitlab.example/acme/skills.git",
        "git:password@gitlab.example:acme/skills.git",
    ] {
        assert!(
            matches!(parse_git_source(input), Err(SourceParseError::InvalidUrl)),
            "credential-bearing source should fail: {input}"
        );
    }

    let ssh = parse_git_source("ssh://git@gitlab.example/acme/skills.git").unwrap();
    assert_eq!(ssh.clone_url, "ssh://git@gitlab.example/acme/skills.git");

    let absolute_scp = parse_git_source("git@gitlab.example:/srv/skills.git")
        .expect("an absolute remote scp path is still a legal source");
    assert_eq!(absolute_scp.clone_url, "git@gitlab.example:/srv/skills.git");
}

#[test]
fn github_reference_requires_a_valid_owner_and_repository_path() {
    for input in [
        "https://github.com",
        "https://github.com/",
        "https://github.com/owner",
        "https://github.com/owner/",
        "https://github.com/owner/repo/",
        "https://github.com/owner/repo/extra",
        "git@github.com:owner/repo/extra.git",
        "git@github.com:/owner/repo.git",
    ] {
        assert!(
            matches!(
                parse_source_reference(SourceKind::GitHub, input, None),
                Err(SourceParseError::InvalidUrl)
            ),
            "invalid GitHub repository path should fail: {input}"
        );
    }

    let valid = parse_source_reference(
        SourceKind::GitHub,
        "https://github.com/owner/repo.git",
        None,
    )
    .expect("owner/repo path should be accepted");
    assert!(matches!(valid, skill_registry::SourceReference::GitHub(_)));
}

#[test]
fn github_repository_validator_rejects_special_characters_and_extra_path_segments() {
    for path in [
        "bad:owner/repo",
        "bad~owner/repo",
        "bad^owner/repo",
        "bad?owner/repo",
        "bad*owner/repo",
        "bad[owner/repo",
        "bad@owner/repo",
        "owner/repo:name",
        "owner/repo~name",
        "owner/repo^name",
        "owner/repo?name",
        "owner/repo*name",
        "owner/repo[name",
        "owner/repo@name",
        "owner/repo/name",
        "owner/repo\\name",
    ] {
        assert!(
            parse_git_source(path).is_err(),
            "invalid shorthand repository path should fail: {path:?}"
        );
        let url = format!("https://github.com/{path}");
        assert!(
            parse_source_reference(SourceKind::GitHub, &url, None).is_err(),
            "invalid GitHub repository path should fail: {url:?}"
        );
    }
}

#[test]
fn source_resolver_rejects_drive_qualified_absolute_and_unc_subpaths() {
    for subpath in [
        "C:/outside",
        "D:outside",
        "c:/outside",
        "d:outside",
        "C:\\outside",
        "\\\\server\\share\\outside",
        "//server/share/outside",
        "\\\\?\\C:\\outside",
        "/outside",
        "\\outside",
    ] {
        let tree_path = format!("main/{subpath}");
        assert!(
            matches!(
                resolve_tree_branch_path(&tree_path, &[]),
                Err(SourceParseError::InvalidSubpath { .. })
            ),
            "absolute subpath should fail: {subpath:?}"
        );
    }
}

#[test]
fn source_resolver_rejects_git_ref_special_characters() {
    for branch in [
        "main:prod",
        "main~prod",
        "main^prod",
        "main?prod",
        "main*prod",
        "main[prod",
        "main\\prod",
        "main..prod",
        "main@{prod}",
        "main prod",
        "main.",
        "main/.hidden",
        "main/foo./bar",
        "@",
    ] {
        let known = vec![branch.to_owned()];
        assert!(
            matches!(
                resolve_tree_branch_path(branch, &known),
                Err(SourceParseError::InvalidBranch)
            ),
            "invalid Git ref should fail: {branch:?}"
        );
    }
}

#[test]
fn source_reference_requires_explicit_kind_and_keeps_well_known_sources_opaque() {
    let well_known = parse_source_reference(
        SourceKind::WellKnown,
        "official/calendar",
        Some("https://official.example/calendar"),
    )
    .unwrap();
    assert!(matches!(
        well_known,
        skill_registry::SourceReference::WellKnown {
            original_input,
            install_url: Some(_)
        } if original_input == "official/calendar"
    ));

    let github = parse_source_reference(
        SourceKind::GitHub,
        "owner/repo",
        Some("https://github.com/owner/repo/tree/main/skills/calendar"),
    )
    .unwrap();
    assert!(matches!(
        github,
        skill_registry::SourceReference::GitHub(source)
            if source.clone_url == "https://github.com/owner/repo.git"
                && source.branch.as_deref() == Some("main")
                && source.subpath.as_deref() == Some("skills/calendar")
    ));

    let mismatch = parse_source_reference(
        SourceKind::GitHub,
        "owner/repo",
        Some("https://gitlab.example/acme/skills.git"),
    );
    assert!(matches!(
        mismatch,
        Err(SourceParseError::SourceKindMismatch { .. })
    ));

    let ssh_github = parse_source_reference(
        SourceKind::GitHub,
        "ssh://git@github.com/owner/repo.git",
        None,
    )
    .unwrap();
    assert!(matches!(
        ssh_github,
        skill_registry::SourceReference::GitHub(source)
            if source.clone_url == "ssh://git@github.com/owner/repo.git"
    ));

    assert!(matches!(
        parse_source_reference(SourceKind::Unknown, "owner/repo", None),
        Err(SourceParseError::UnsupportedSourceKind { .. })
    ));
}

#[test]
fn client_validates_query_limit_base_url_proxy_and_debug_boundaries() {
    assert!(matches!(
        SkillsShClient::new().unwrap().search_url("", 1),
        Err(RegistryError::InvalidQuery {
            reason: QueryValidationError::Empty
        })
    ));
    assert!(matches!(
        SkillsShClient::new().unwrap().search_url("a", 1),
        Err(RegistryError::InvalidQuery {
            reason: QueryValidationError::TooShort { minimum }
        }) if minimum == MIN_QUERY_LENGTH
    ));
    assert!(SkillsShClient::new().unwrap().search_url("ab", 1).is_ok());
    assert!(matches!(
        SkillsShClient::new().unwrap().search_url("query", 0),
        Err(RegistryError::InvalidLimit { limit: 0, .. })
    ));
    assert!(matches!(
        SkillsShClient::new().unwrap().search_url("query", 301),
        Err(RegistryError::InvalidLimit { limit: 301, .. })
    ));
    assert!(matches!(
        SkillsShClient::builder()
            .base_url("file:///tmp/registry")
            .build(),
        Err(RegistryError::UnsupportedBaseUrlScheme)
    ));
    assert!(matches!(
        SkillsShClient::builder()
            .base_url("http:///registry")
            .build(),
        Err(RegistryError::InvalidBaseUrl)
    ));
    assert!(matches!(
        SkillsShClient::builder()
            .base_url("http://registry.example/%2e%2e/")
            .build(),
        Err(RegistryError::InvalidBaseUrl)
    ));
    assert!(matches!(
        SkillsShClient::builder()
            .base_url("http://@registry.example/")
            .build(),
        Err(RegistryError::InvalidBaseUrl)
    ));

    for proxy in ["file:///tmp/proxy", "http:///proxy", "not a URL"] {
        assert!(matches!(
            SkillsShClient::builder().proxy_url(proxy).build(),
            Err(RegistryError::InvalidProxy)
        ));
    }
    assert!(SkillsShClient::builder()
        .proxy_url("http://proxy.example")
        .build()
        .is_ok());

    let debug = format!(
        "{:?}",
        SkillsShClient::builder()
            .proxy_url("http://proxy-user:proxy-secret@proxy.example")
            .user_agent("caller-sensitive-user-agent")
    );
    assert!(debug.contains("user_agent_configured: true"));
    assert!(!debug.contains("caller-sensitive-user-agent"));
    assert!(!debug.contains("proxy-secret"));
}
