//! Whole trader runs against the scripted model in `mock_model`.

use super::{
    mock_model::{MockModel, Reply, REPLY_TOKENS},
    service::RaceRunContext,
    ModelService,
};
use crate::{
    database::Database,
    mcp::{repositories::LiveQuote, services::tools::FrozenMarket},
};
use dtos::ai_models::{
    AiRun, AiRunEvent, AiRunStatus, ModelProfileSettings, NewAiRace, NewAiTrader,
    NewModelConnection, MODEL_RETRY_EVENT,
};
use rust_decimal_macros::dec;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

const API_KEY: &str = "sk-mock-0123456789abcdef";
/// The user's own portfolio in `service_with_database`.
const MAIN: &str = "11111111-1111-1111-1111-111111111111";

fn service() -> ModelService {
    let db = Database::in_memory().unwrap();
    let connector = crate::mcp::services::tests::service_with_database(db.clone());
    ModelService::new(db, connector)
}

/// A $5,000 AI portfolio whose model is `model` on the mock endpoint.
async fn trader(service: &ModelService, mock: &MockModel, model: &str) -> Uuid {
    service
        .create_trader(NewAiTrader {
            name: model.into(),
            starting_cash: dec!(5000),
            profile_id: None,
            connection: Some(NewModelConnection {
                name: model.into(),
                base_url: mock.base_url().into(),
                model: model.into(),
                api_key: API_KEY.into(),
            }),
            strategy: "Buy quality growth.".into(),
        })
        .await
        .unwrap()
        .portfolio_id
}

/// Waits for the run to finish.
async fn settle(service: &ModelService, run: AiRun) -> AiRun {
    for _ in 0..500 {
        let run = service.run(run.id).unwrap();
        if run.status != AiRunStatus::Running {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("run {} never finished", run.id)
}

async fn run_once(service: &ModelService, portfolio: Uuid) -> AiRun {
    let run = service.start_run(portfolio).await.unwrap();
    settle(service, run).await
}

fn error(run: &AiRun) -> &str {
    run.error.as_deref().unwrap_or_default()
}

fn tool_names(request: &Value) -> Vec<String> {
    let mut names: Vec<String> = request["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn a_full_run_trades_journals_remembers_and_counts_tokens() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "full").await;
    let reason = "Data-centre demand keeps growing.";
    mock.script(
        "full",
        [
            Reply::tools(&[("get_my_portfolio", json!({}))]),
            // Names the user's portfolio: the order must still land on the trader's.
            Reply::tools(&[(
                "place_order",
                json!({"portfolio": MAIN, "ticker": "NVDA", "side": "buy", "amount_usd": 1000, "reason": reason}),
            )]),
            Reply::text("Bought $1,000 of NVDA."),
            Reply::text(r#"{"decision_summary":"Holds NVDA for AI demand.","unresolved_questions":"Is the valuation stretched?"}"#),
        ],
    );

    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("Bought $1,000 of NVDA."));
    let (prompt, completion) = REPLY_TOKENS;
    assert_eq!(run.prompt_tokens, Some(4 * prompt));
    assert_eq!(run.completion_tokens, Some(4 * completion));
    assert_eq!(run.total_tokens, Some(4 * (prompt + completion)));
    let tools: Vec<_> = run.events.iter().map(|e| (e.tool.as_str(), e.success)).collect();
    assert_eq!(tools, [("get_my_portfolio", true), ("place_order", true)]);

    let book = service.mcp.trading().book(Some(&portfolio.to_string())).unwrap();
    let bought = book.transactions.iter().find(|t| t.ticker.as_str() == "NVDA").expect("an NVDA buy");
    assert_eq!(bought.shares, dec!(5), "$1,000 at $200");
    let main_trades: i64 = service
        .db
        .with(|c| c.query_row("SELECT COUNT(*) FROM transactions WHERE portfolio_id=?1", [MAIN], |r| r.get(0)))
        .unwrap();
    assert_eq!(main_trades, 1, "the user's portfolio is untouched");
    let journal: Vec<(String, String)> = service
        .db
        .with(|c| {
            c.prepare("SELECT author, text FROM thesis_log WHERE portfolio_id=?1 AND ticker='NVDA'")?
                .query_map([portfolio.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
        .unwrap();
    assert!(journal.iter().any(|(_, text)| text.contains(reason)), "{journal:?}");

    let memory = service.memory(portfolio).unwrap();
    assert_eq!(memory.decision_summary, "Holds NVDA for AI demand.");
    assert_eq!(memory.unresolved_questions, "Is the valuation stretched?");

    let requests = mock.received("full");
    assert_eq!(requests.len(), 4);
    let expected = format!("Bearer {API_KEY}");
    assert!(requests.iter().all(|r| r.header("authorization") == Some(expected.as_str())));
    assert_eq!(requests[0].body["model"], "full");
    assert_eq!(
        tool_names(&requests[0].body),
        ["get_my_portfolio", "get_quote", "get_thesis", "list_theses", "place_order"]
    );
    let schemas = requests[0].body["tools"].to_string();
    assert!(!schemas.contains("\"portfolio\""), "the model is never asked for a portfolio");
    let messages = requests[1].body["messages"].as_array().unwrap();
    let last = messages.last().unwrap();
    assert_eq!(last["role"], "tool");
    assert_eq!(last["tool_call_id"], "call_0");
    assert!(last["content"].as_str().unwrap().contains("5000"), "the tool result goes back to the model");
    assert!(requests[3].body.get("tools").is_none(), "the memory request offers no tools");
}

#[tokio::test]
async fn provider_errors_fail_the_run_without_leaking_the_key() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "errors").await;
    let cases = [
        (
            Reply::error(401, &format!("Incorrect API key provided: {API_KEY}")),
            "Model endpoint returned 401: Incorrect API key provided: ***",
        ),
        (Reply::error(400, "Unknown parameter."), "Model endpoint returned 400: Unknown parameter."),
        (Reply::error(403, "Not allowed."), "Model endpoint returned 403: Not allowed."),
        (Reply::Raw(404, "<html>Not found</html>".into()), "Model endpoint returned 404."),
        (Reply::Raw(200, "not json".into()), "The model endpoint returned invalid JSON."),
        (Reply::Ok(json!({"choices": []})), "The model returned no assistant message."),
        (Reply::text("  "), "The model finished without a response."),
    ];
    for (reply, expected) in cases {
        let before = mock.received("errors").len();
        mock.script("errors", [reply]);
        let run = run_once(&service, portfolio).await;
        assert_eq!(run.status, AiRunStatus::Failed, "{expected}");
        assert_eq!(error(&run), expected);
        assert!(!error(&run).contains(API_KEY));
        assert_eq!(mock.received("errors").len(), before + 1, "{expected}: not retried");
        assert!(retries(&run).is_empty());
    }
}

fn retries(run: &AiRun) -> Vec<&AiRunEvent> {
    run.events.iter().filter(|e| e.tool == MODEL_RETRY_EVENT).collect()
}

#[tokio::test]
async fn rate_limits_and_server_errors_are_retried() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "flaky").await;
    mock.script(
        "flaky",
        [
            Reply::error(429, "Rate limit reached.").header("retry-after-ms", "50"),
            Reply::Raw(503, "Service unavailable".into()),
            Reply::text("No trades today."),
            Reply::text("{}"),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(mock.received("flaky").len(), 4, "two retries, the answer and memory");
    let (prompt, completion) = REPLY_TOKENS;
    assert_eq!(run.total_tokens, Some(2 * (prompt + completion)), "failed tries report no usage");
    let retried = retries(&run);
    assert_eq!(retried.len(), 2);
    assert!(retried.iter().all(|e| !e.success));
    assert!(retried[0].detail.starts_with("Model endpoint returned 429: Rate limit reached. Trying again in"), "{}", retried[0].detail);
    assert!(retried[0].detail.ends_with("(attempt 2 of 4)."), "{}", retried[0].detail);
    let waited: Value = serde_json::from_str(&retried[0].arguments).unwrap();
    assert_eq!(waited, json!({"attempt": 2, "wait_ms": 50}), "Retry-After is honoured");
    assert!(retried[1].detail.starts_with("Model endpoint returned 503."), "{}", retried[1].detail);
    assert!(retried[1].detail.ends_with("(attempt 3 of 4)."));

    for status in [429, 500, 502, 503, 504] {
        let model = format!("down-{status}");
        let portfolio = trader(&service, &mock, &model).await;
        mock.script(&model, (0..4).map(|_| Reply::error(status, "Try later.")));
        let run = run_once(&service, portfolio).await;
        assert_eq!(
            error(&run),
            format!("Model endpoint returned {status}: Try later. Gave up after 4 attempts.")
        );
        assert_eq!(mock.received(&model).len(), 4);
        assert_eq!(retries(&run).len(), 3);
    }
}

#[tokio::test]
async fn retries_stop_before_the_deadline() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "slow-down").await;
    // Asked to wait longer than the run may take: fail now.
    mock.script("slow-down", [Reply::error(429, "Slow down.").header("retry-after", "300")]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(
        error(&run),
        "Model endpoint returned 429: Slow down. Not retried: the run's deadline comes first."
    );
    assert_eq!(mock.received("slow-down").len(), 1);
    assert!(retries(&run).is_empty());

    // A short deadline, as in a race round: one wait fits, the next doesn't.
    mock.script(
        "slow-down",
        [
            Reply::error(429, "Slow down.").header("retry-after", "1"),
            Reply::error(429, "Slow down.").header("retry-after", "1.5"),
        ],
    );
    let run = service.start_run_for(portfolio, None, Duration::from_secs(2)).await.unwrap();
    let run = settle(&service, run).await;
    assert_eq!(
        error(&run),
        "Model endpoint returned 429: Slow down. Gave up after 2 attempts: the run's deadline comes first."
    );
    assert_eq!(retries(&run).len(), 1);
}

#[tokio::test]
async fn dropped_connections_are_retried_but_profile_tests_are_not() {
    let service = service();
    let closed = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let portfolio = service
        .create_trader(NewAiTrader {
            name: "Offline".into(),
            starting_cash: dec!(5000),
            profile_id: None,
            connection: Some(NewModelConnection {
                name: "Offline".into(),
                base_url: format!("http://{closed}/v1"),
                model: "offline".into(),
                api_key: API_KEY.into(),
            }),
            strategy: "Buy quality growth.".into(),
        })
        .await
        .unwrap()
        .portfolio_id;
    let run = run_once(&service, portfolio).await;
    assert!(error(&run).starts_with("Model request failed: "), "{}", error(&run));
    assert!(error(&run).ends_with(" Gave up after 4 attempts."), "{}", error(&run));
    assert_eq!(retries(&run).len(), 3);

    // Testing a profile answers at once, whatever went wrong.
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "busy").await;
    mock.script("busy", [Reply::error(429, "Rate limit reached.")]);
    let profile = service.config(portfolio).unwrap().profile_id.unwrap();
    let error = service.test_profile(profile).await.unwrap_err().to_string();
    assert!(error.contains("Model endpoint returned 429: Rate limit reached."), "{error}");
    assert!(!error.contains("attempt"), "{error}");
    assert_eq!(mock.received("busy").len(), 1);
}

#[tokio::test]
async fn misbehaving_models_are_stopped() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "unruly").await;
    let quote = || ("get_quote", json!({"ticker": "NVDA"}));

    mock.script("unruly", [Reply::tools(&[("save_thesis", json!({"ticker": "NVDA"}))])]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model requested unavailable tool save_thesis.");
    assert!(run.events.is_empty(), "nothing ran");

    mock.script(
        "unruly",
        [Reply::message(json!({
            "role": "assistant",
            "tool_calls": [{"id": "x", "type": "function", "function": {"name": "get_quote", "arguments": "{not json"}}]
        }))],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model returned invalid arguments for get_quote.");

    let calls: Vec<_> = (0..25).map(|_| quote()).collect();
    mock.script("unruly", [Reply::tools(&calls)]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model exceeded the 24 tool-call limit.");
    assert_eq!(run.events.len(), 24);

    mock.script("unruly", (0..12).map(|_| Reply::tools(&[quote()])));
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model exceeded the 12-turn limit.");
    assert_eq!(run.events.len(), 12);

    // A failed tool call goes back to the model, which can carry on.
    mock.script(
        "unruly",
        [
            Reply::tools(&[("get_quote", json!({"ticker": "NOPE"}))]),
            Reply::text("No such ticker; no trades."),
            Reply::text("{}"),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert!(!run.events[0].success);
    // The last request updates memory; the one before carries the tool result.
    let sent = mock.received("unruly");
    let reply = sent[sent.len() - 2].body["messages"].as_array().unwrap().last().cloned().unwrap();
    assert_eq!(reply["role"], "tool");
    assert!(reply["content"].as_str().unwrap().contains("NOPE"), "{reply}");
}

#[tokio::test]
async fn race_contestants_share_one_frozen_market_and_late_ones_fail() {
    let service = service();
    let mock = MockModel::start().await;
    let fast = trader(&service, &mock, "fast").await;
    let slow = trader(&service, &mock, "slow").await;
    let race = service
        .create_race(NewAiRace {
            name: "Mock race".into(),
            contestant_ids: vec![fast, slow],
            starting_capital: 5_000.0,
            rounds: 1,
            trading_frequency_minutes: 1,
            round_timeout_seconds: 15,
        })
        .unwrap();
    // Running, without the race loop: the test drives the round itself.
    service
        .db
        .with(|c| c.execute("UPDATE ai_races SET status='running' WHERE id=?1", [race.id.to_string()]))
        .unwrap();
    // The live price is $200; the round froze it at $150.
    let market = FrozenMarket::new();
    market
        .freeze(
            "NVDA",
            LiveQuote {
                price: dec!(150),
                previous_close: dec!(149),
                currency: "USD".into(),
                usd_per_unit: dec!(1),
                timestamp: 1_767_600_000,
                stale: false,
            },
        )
        .await;
    let buy = || Reply::tools(&[("place_order", json!({"ticker": "NVDA", "side": "buy", "shares": 2, "reason": "Race entry."}))]);
    mock.script("fast", [buy(), Reply::text("Bought 2 NVDA."), Reply::text("{}")]);
    mock.script("slow", [buy(), Reply::text("Too late.").after(Duration::from_secs(10))]);

    let deadline = Duration::from_secs(2);
    let mut runs = vec![];
    for portfolio in [fast, slow] {
        let context = RaceRunContext { race_id: race.id, round: 1, market: market.clone() };
        runs.push(service.start_run_for(portfolio, Some(context), deadline).await.unwrap());
    }
    let slow_run = settle(&service, runs.pop().unwrap()).await;
    let fast_run = settle(&service, runs.pop().unwrap()).await;
    assert_eq!(fast_run.status, AiRunStatus::Completed, "{fast_run:?}");
    assert_eq!(slow_run.status, AiRunStatus::Failed);
    assert_eq!(error(&slow_run), "The model run exceeded the 2 second round deadline.");
    assert_eq!(slow_run.total_tokens, Some(REPLY_TOKENS.0 + REPLY_TOKENS.1), "tokens used before the deadline count");

    for portfolio in [fast, slow] {
        let book = service.mcp.trading().book(Some(&portfolio.to_string())).unwrap();
        let bought = book.transactions.iter().find(|t| t.ticker.as_str() == "NVDA").expect("an NVDA buy");
        assert_eq!(bought.price, dec!(150), "filled at the frozen price, not the live one");
    }
}

/// Saves `settings` on the profile of `portfolio`'s trader.
fn configure(service: &ModelService, mock: &MockModel, portfolio: Uuid, settings: ModelProfileSettings) -> Uuid {
    let id = service.config(portfolio).unwrap().profile_id.unwrap();
    let profile = service.profiles().unwrap().into_iter().find(|p| p.id == id).unwrap();
    service
        .save_profile_with(Some(id), &profile.name, mock.base_url(), &profile.model, None, Some(&settings))
        .unwrap();
    id
}

#[tokio::test]
async fn profile_settings_shape_every_request_and_stay_secret() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "custom").await;
    let secret = "org-secret-0123456789";
    let id = configure(
        &service,
        &mock,
        portfolio,
        ModelProfileSettings {
            timeout_seconds: 30,
            max_tokens: Some(512),
            key_header: "API-Key".into(),
            extra_headers: Some(vec![
                ("OpenAI-Organization".into(), secret.into()),
                ("X-Project".into(), "proj-7".into()),
            ]),
        },
    );
    let profile = service.profiles().unwrap().into_iter().find(|p| p.id == id).unwrap();
    assert_eq!(profile.timeout_seconds, 30);
    assert_eq!(profile.max_tokens, Some(512));
    assert_eq!(profile.key_header, "api-key");
    assert_eq!(profile.extra_header_names, ["openai-organization", "x-project"]);

    mock.script("custom", [Reply::text("Nothing to do."), Reply::text("{}")]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    mock.script("custom", [Reply::tools(&[("connection_test", json!({}))])]);
    service.test_profile(id).await.unwrap();
    let requests = mock.received("custom");
    assert_eq!(requests.len(), 3, "the run, its memory and the profile test");
    for request in &requests {
        assert_eq!(request.header("api-key"), Some(API_KEY));
        assert_eq!(request.header("authorization"), None, "the key goes only where asked");
        assert_eq!(request.header("openai-organization"), Some(secret));
        assert_eq!(request.header("x-project"), Some("proj-7"));
        assert_eq!(request.body["max_tokens"], 512);
    }

    // Header values never leave the server: not in the profile list, not
    // in a backup, not in an error the provider echoes them in.
    let listed = serde_json::to_string(&service.profiles().unwrap()).unwrap();
    assert!(!listed.contains(secret) && !listed.contains(API_KEY), "{listed}");
    let backup = service.db.export().unwrap();
    assert!(!backup.windows(secret.len()).any(|w| w == secret.as_bytes()));
    mock.script("custom", [Reply::error(403, &format!("Organization {secret} is not allowed."))]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "Model endpoint returned 403: Organization *** is not allowed.");

    // Saving the connection alone keeps the settings; `None` keeps the
    // headers, and an empty list removes them.
    service.save_profile(Some(id), "Renamed", mock.base_url(), "custom", None).unwrap();
    let kept = |service: &ModelService| service.profiles().unwrap().into_iter().find(|p| p.id == id).unwrap();
    assert_eq!(kept(&service).timeout_seconds, 30);
    assert_eq!(kept(&service).extra_header_names.len(), 2);
    let mut settings = ModelProfileSettings { timeout_seconds: 30, ..Default::default() };
    configure(&service, &mock, portfolio, settings.clone());
    assert_eq!(kept(&service).extra_header_names.len(), 2);
    assert_eq!(kept(&service).key_header, "", "back to Authorization: Bearer");
    settings.extra_headers = Some(vec![]);
    configure(&service, &mock, portfolio, settings);
    assert!(kept(&service).extra_header_names.is_empty());
}

#[tokio::test]
async fn a_slow_answer_times_out_and_is_retried() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "sleepy").await;
    configure(&service, &mock, portfolio, ModelProfileSettings { timeout_seconds: 1, ..Default::default() });
    mock.script(
        "sleepy",
        [
            Reply::text("Too late.").after(Duration::from_secs(3)),
            Reply::text("On time."),
            Reply::text("{}"),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("On time."));
    let retried = retries(&run);
    assert_eq!(retried.len(), 1);
    assert!(
        retried[0].detail.starts_with("The model didn't answer within 1 s. Trying again in"),
        "{}",
        retried[0].detail
    );
}
