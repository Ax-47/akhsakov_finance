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
    NewModelConnection, TraderConfig, MODEL_RETRY_EVENT,
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
        [
            "add_thesis_note",
            "get_my_goals",
            "get_my_portfolio",
            "get_quote",
            "get_thesis",
            "list_theses",
            "place_order",
            "save_thesis"
        ]
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

    mock.script("unruly", [Reply::tools(&[("create_alert", json!({"ticker": "NVDA"}))])]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model requested unavailable tool create_alert.");
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
                hand_priced: false,
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
            ..Default::default()
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
    let probe = || Reply::tools(&[("get_price", json!({"ticker": "NVDA"}))]);
    mock.script("custom", [probe(), probe()]);
    service.test_profile(id).await.unwrap();
    let requests = mock.received("custom");
    assert_eq!(requests.len(), 4, "the run, its memory and the profile test's two questions");
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

#[tokio::test]
async fn traders_keep_their_own_theses_and_read_their_goals() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "thinker").await;
    let mine = portfolio.to_string();
    service
        .db
        .with(|c| {
            c.execute(
                "INSERT INTO goals (id, name, target, date, monthly, portfolio_id) VALUES ('33333333-3333-3333-3333-333333333333', 'Double it', '10000', '2030-01-01', '0', ?1)",
                [&mine],
            )?;
            c.execute(
                "INSERT INTO goals (id, name, target, date, monthly, portfolio_id) VALUES ('44444444-4444-4444-4444-444444444444', 'House', '50000', '2030-01-01', '0', ?1)",
                [MAIN],
            )
        })
        .unwrap();
    // Every call names the user's portfolio; all of them must land on the trader's.
    mock.script(
        "thinker",
        [
            Reply::tools(&[
                ("save_thesis", json!({"portfolio": MAIN, "ticker": "NVDA", "thesis": "AI demand.", "conviction": 4, "note": "Started."})),
                ("add_thesis_note", json!({"portfolio": MAIN, "ticker": "NVDA", "text": "Checked the data-centre numbers."})),
                ("get_my_goals", json!({"portfolio": MAIN})),
            ]),
            Reply::text("Wrote the NVDA thesis."),
            Reply::text("{}"),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert!(run.events.iter().all(|e| e.success), "{:?}", run.events);

    let theses: Vec<(String, String)> = service
        .db
        .with(|c| {
            c.prepare("SELECT portfolio_id, thesis FROM theses WHERE ticker='NVDA'")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect()
        })
        .unwrap();
    assert_eq!(theses, [(mine.clone(), "AI demand.".to_string())], "only the trader's portfolio");
    let journal: Vec<(String, String, String)> = service
        .db
        .with(|c| {
            c.prepare("SELECT portfolio_id, author, text FROM thesis_log WHERE ticker='NVDA' ORDER BY created_at")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect()
        })
        .unwrap();
    assert!(journal.iter().all(|(p, author, _)| *p == mine && author.eq_ignore_ascii_case("ai")), "{journal:?}");
    assert!(journal.iter().any(|(_, _, text)| text == "Checked the data-centre numbers."), "{journal:?}");

    let requests = mock.received("thinker");
    let system = requests[0].body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("save_thesis") && system.contains("get_my_goals"), "{system}");
    let goals = requests[1].body["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string();
    assert!(goals.contains("Double it") && !goals.contains("House"), "{goals}");
}

#[tokio::test]
async fn each_run_starts_with_the_mandate_and_goals() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "mandated").await;
    let mut config = service.config(portfolio).unwrap();
    for bad in [
        TraderConfig { horizon_years: Some(0), ..config.clone() },
        TraderConfig { horizon_years: Some(51), ..config.clone() },
        TraderConfig { max_drawdown_pct: Some(0), ..config.clone() },
        TraderConfig { max_drawdown_pct: Some(101), ..config.clone() },
        TraderConfig { benchmark: "not a ticker!".into(), ..config.clone() },
    ] {
        assert!(service.save_config(bad).is_err());
    }

    // No mandate and no goals yet.
    mock.script("mandated", [Reply::text("Nothing to do."), Reply::text("{}")]);
    run_once(&service, portfolio).await;
    let system = |n: usize| mock.received("mandated")[n].body["messages"][0]["content"].as_str().unwrap().to_string();
    let first = system(0);
    assert!(first.contains("- None beyond the strategy."), "{first}");
    assert!(first.contains("No goals set for this portfolio."), "{first}");

    config.horizon_years = Some(10);
    config.max_drawdown_pct = Some(20);
    config.benchmark = " spy ".into();
    let saved = service.save_config(config).unwrap();
    assert_eq!((saved.horizon_years, saved.max_drawdown_pct, saved.benchmark.as_str()), (Some(10), Some(20), "SPY"));
    service
        .db
        .with(|c| {
            c.execute(
                "INSERT INTO goals (id, name, target, date, monthly, portfolio_id)
                 VALUES ('55555555-5555-5555-5555-555555555555', 'Retire early', '20000', '2040-01-01', '100', ?1)",
                [portfolio.to_string()],
            )
        })
        .unwrap();
    mock.script("mandated", [Reply::text("Nothing to do."), Reply::text("{}")]);
    run_once(&service, portfolio).await;
    let second = system(2);
    assert!(second.contains("- Invest for a horizon of about 10 years."), "{second}");
    assert!(second.contains("within 20%"), "{second}");
    assert!(second.contains("Measure yourself against SPY"), "{second}");
    let goals = second.split("<goals>").nth(1).and_then(|g| g.split("</goals>").next()).unwrap();
    let goals: Value = serde_json::from_str(goals.trim()).unwrap();
    assert_eq!(goals["goals"][0]["name"], "Retire early");
    assert!(goals["goals"][0]["on_track"].is_boolean(), "{goals}");
    assert_eq!(goals["portfolio_value"], 5000.0);
}

#[tokio::test]
async fn the_emergency_stop_halts_runs_races_and_orders_and_survives_a_restart() {
    use dtos::{ai_models::AI_STOPPED_MESSAGE, mcp::McpAccessPreset};
    let service = service();
    let mock = MockModel::start().await;
    let runner = trader(&service, &mock, "stopped").await;
    let racer = trader(&service, &mock, "racer").await;
    let rival = trader(&service, &mock, "rival").await;
    let bot = service.mcp.create_connection("Bot", McpAccessPreset::Trader, vec![runner]).unwrap();

    // A run waiting on a slow model, and a race between two other traders.
    mock.script("stopped", [Reply::text("Too late.").after(Duration::from_secs(30))]);
    let pending = service.start_run(runner).await.unwrap();
    let race = service
        .create_race(NewAiRace {
            name: "Halted".into(),
            contestant_ids: vec![racer, rival],
            starting_capital: 5_000.0,
            rounds: 3,
            trading_frequency_minutes: 1,
            round_timeout_seconds: 15,
        })
        .unwrap();
    service
        .db
        .with(|c| c.execute("UPDATE ai_races SET status='running' WHERE id=?1", [race.id.to_string()]))
        .unwrap();

    let state = service.set_ai_stop(true, "misha").unwrap();
    assert!(state.active);
    assert_eq!((state.history[0].actor.as_str(), state.history[0].active), ("misha", true));
    let ended = settle(&service, pending).await;
    assert_eq!(ended.status, AiRunStatus::Failed);
    assert_eq!(error(&ended), "Stopped by the emergency stop.");
    assert_eq!(service.race(race.id).unwrap().status, dtos::ai_models::AiRaceStatus::Paused);

    let refused = |e: crate::shared::ServiceError| assert_eq!(e.to_string(), crate::shared::ServiceError::Validation(AI_STOPPED_MESSAGE.into()).to_string());
    refused(service.start_run(runner).await.unwrap_err());
    refused(service.resume_race(race.id).unwrap_err());
    let crate::mcp::services::Access::Granted(access) = service.mcp.authorize(Some(&bot.token)) else {
        panic!("the connection should authenticate")
    };
    let order = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"place_order","arguments":{"ticker":"NVDA","side":"buy","shares":1,"reason":"test"}}});
    let reply = service.mcp.handle_scoped(order.clone(), &access).await.unwrap();
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    assert!(reply.to_string().contains("emergency stop"), "{reply}");

    // A restart keeps the stop on.
    let restarted = ModelService::new(service.db.clone(), service.mcp.clone());
    assert!(restarted.ai_stop().unwrap().active);
    refused(restarted.start_run(runner).await.unwrap_err());

    // Off again: orders and runs go ahead; the race waits to be resumed.
    let state = restarted.set_ai_stop(false, "misha").unwrap();
    assert!(!state.active);
    assert_eq!(state.history.len(), 2);
    let reply = service.mcp.handle_scoped(order, &access).await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(restarted.race(race.id).unwrap().status, dtos::ai_models::AiRaceStatus::Paused);
    restarted.stop_race(race.id).unwrap();
}

#[tokio::test]
async fn scheduled_runs_start_once_per_slot_and_skip_when_they_must() {
    use chrono::{Duration as Hours, Utc};
    use dtos::ai_models::{MarketHours, TraderSchedule};
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "scheduled").await;
    let base = TraderSchedule { enabled: true, every_hours: 1, ..TraderSchedule::new(portfolio) };
    assert!(service.save_schedule(TraderSchedule { every_hours: 0, ..base.clone() }).is_err());
    assert!(service.save_schedule(TraderSchedule { every_hours: 169, ..base.clone() }).is_err());
    assert!(service.save_schedule(TraderSchedule { weekdays: vec![], ..base.clone() }).is_err());
    assert!(service.save_schedule(TraderSchedule { weekdays: vec![8], ..base.clone() }).is_err());
    let saved = service
        .save_schedule(TraderSchedule { weekdays: vec![3, 1, 1, 2, 4, 5, 6, 7], ..base.clone() })
        .unwrap();
    assert_eq!(saved.weekdays, [1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(saved.market_hours, MarketHours::Any);
    assert!(saved.next_run_at.is_some());

    let due = |at: chrono::DateTime<Utc>| {
        service
            .db
            .with(|c| {
                c.execute(
                    "UPDATE ai_trader_schedules SET next_run_at=?2 WHERE portfolio_id=?1",
                    [portfolio.to_string(), at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)],
                )
            })
            .unwrap();
    };
    let now = Utc::now();
    // Due since three hours ago, as if the app had been off: one run, not three.
    due(now - Hours::hours(3));
    mock.script("scheduled", [Reply::text("Nothing to do."), Reply::text("{}")]);
    assert_eq!(service.run_due_schedules(now).await.unwrap(), 1);
    let runs = service.runs(portfolio).unwrap();
    assert_eq!(runs.len(), 1);
    settle(&service, runs[0].clone()).await;
    let after = service.schedule(portfolio).unwrap();
    assert!(after.next_run_at.as_deref() > Some(now.to_rfc3339().as_str()), "{after:?}");
    assert!(after.last_run_at.is_some() && after.last_note.is_none(), "{after:?}");
    // The slot is taken: neither this service nor a restarted one runs it again.
    assert_eq!(service.run_due_schedules(now).await.unwrap(), 0);
    let restarted = ModelService::new(service.db.clone(), service.mcp.clone());
    assert_eq!(restarted.run_due_schedules(now).await.unwrap(), 0);

    // The emergency stop skips a due run and moves the schedule on.
    service.set_ai_stop(true, "misha").unwrap();
    due(now - Hours::minutes(1));
    assert_eq!(service.run_due_schedules(now).await.unwrap(), 0);
    let skipped = service.schedule(portfolio).unwrap();
    assert!(skipped.last_note.as_deref().unwrap().contains("emergency stop"), "{skipped:?}");
    assert!(skipped.next_run_at.as_deref() > Some(now.to_rfc3339().as_str()));
    service.set_ai_stop(false, "misha").unwrap();

    // So does an active race.
    let rival = trader(&service, &mock, "rival").await;
    let race = service
        .create_race(NewAiRace {
            name: "Busy".into(),
            contestant_ids: vec![portfolio, rival],
            starting_capital: 5_000.0,
            rounds: 1,
            trading_frequency_minutes: 1,
            round_timeout_seconds: 15,
        })
        .unwrap();
    service
        .db
        .with(|c| c.execute("UPDATE ai_races SET status='paused' WHERE id=?1", [race.id.to_string()]))
        .unwrap();
    due(now - Hours::minutes(1));
    assert_eq!(service.run_due_schedules(now).await.unwrap(), 0);
    let skipped = service.schedule(portfolio).unwrap();
    assert!(skipped.last_note.as_deref().unwrap().contains("active race"), "{skipped:?}");
    assert_eq!(service.runs(portfolio).unwrap().len(), 1, "no run started");

    // Turned off, nothing is due.
    let off = service.save_schedule(TraderSchedule { enabled: false, ..base }).unwrap();
    assert!(off.next_run_at.is_none());
}

#[tokio::test]
async fn runs_and_races_notify_at_the_chosen_level() {
    use crate::notifications::{infrastructures::{HttpPusher, SqliteNotificationRepository}, NotificationService};
    use dtos::ai_models::AiNotifyLevel;
    use std::sync::Arc;
    let db = Database::in_memory().unwrap();
    let connector = crate::mcp::services::tests::service_with_database(db.clone());
    let notifications = NotificationService::new(
        Arc::new(SqliteNotificationRepository::new(db.clone())),
        Arc::new(HttpPusher::new()),
    );
    let service = ModelService::new(db, connector).with_notifications(notifications.clone());
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "noisy").await;
    let wait_for = |count: usize| {
        let notifications = notifications.clone();
        async move {
            for _ in 0..100 {
                let all = notifications.recent().unwrap();
                if all.len() >= count {
                    return all;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            notifications.recent().unwrap()
        }
    };
    let buy = || Reply::tools(&[("place_order", json!({"ticker": "NVDA", "side": "buy", "shares": 1, "reason": "Test."}))]);

    // By default only failures notify: a run that trades says nothing.
    assert_eq!(service.notify_level().unwrap(), AiNotifyLevel::Failures);
    mock.script("noisy", [buy(), Reply::text("Bought one NVDA.\nMore detail."), Reply::text("{}")]);
    run_once(&service, portfolio).await;
    mock.script("noisy", [Reply::error(401, "Bad key.")]);
    run_once(&service, portfolio).await;
    let sent = wait_for(1).await;
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].title, "🤖 AI run failed: noisy");
    assert_eq!(sent[0].body, "Model endpoint returned 401: Bad key.");

    // With everything on, a run that trades is summed up in one line.
    service.set_notify_level(AiNotifyLevel::All).unwrap();
    mock.script("noisy", [buy(), Reply::text("Bought one NVDA.\nMore detail."), Reply::text("{}")]);
    run_once(&service, portfolio).await;
    let sent = wait_for(2).await;
    assert_eq!(sent.len(), 2, "{sent:?}");
    let traded = sent.iter().find(|n| n.title.contains("traded")).expect("a trade notification");
    assert_eq!(traded.title, "🤖 AI run traded: noisy");
    assert_eq!(traded.body, "1 trade. Bought one NVDA.");

    // The emergency stop doesn't notify about the runs it ends.
    mock.script("noisy", [Reply::text("Late.").after(Duration::from_secs(30))]);
    let pending = service.start_run(portfolio).await.unwrap();
    service.set_ai_stop(true, "misha").unwrap();
    settle(&service, pending).await;
    service.set_ai_stop(false, "misha").unwrap();

    // Off means nothing at all.
    service.set_notify_level(AiNotifyLevel::Off).unwrap();
    mock.script("noisy", [Reply::error(401, "Bad key.")]);
    run_once(&service, portfolio).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(notifications.recent().unwrap().len(), 2);
}

#[tokio::test]
async fn race_rounds_and_endings_are_summed_up() {
    use crate::notifications::{infrastructures::{HttpPusher, SqliteNotificationRepository}, NotificationService};
    use std::sync::Arc;
    let db = Database::in_memory().unwrap();
    let connector = crate::mcp::services::tests::service_with_database(db.clone());
    let notifications = NotificationService::new(
        Arc::new(SqliteNotificationRepository::new(db.clone())),
        Arc::new(HttpPusher::new()),
    );
    let service = ModelService::new(db, connector).with_notifications(notifications.clone());
    let mock = MockModel::start().await;
    let (a, b) = (trader(&service, &mock, "alpha").await, trader(&service, &mock, "beta").await);
    let race = service
        .create_race(NewAiRace {
            name: "Cup".into(),
            contestant_ids: vec![a, b],
            starting_capital: 5_000.0,
            rounds: 2,
            trading_frequency_minutes: 1,
            round_timeout_seconds: 15,
        })
        .unwrap();
    service
        .db
        .with(|c| {
            for (portfolio, status) in [(a, "failed"), (b, "completed")] {
                c.execute(
                    "INSERT INTO ai_runs (id, portfolio_id, status, profile_name, model, race_id, race_round)
                     VALUES (?1, ?2, ?3, 'p', 'm', ?4, 1)",
                    rusqlite::params![Uuid::new_v4().to_string(), portfolio.to_string(), status, race.id.to_string()],
                )?;
            }
            c.execute("UPDATE ai_races SET status='completed' WHERE id=?1", [race.id.to_string()])
        })
        .unwrap();
    service.announce_round(race.id, 1);
    service.announce_race_paused(race.id, "It stopped on an error: boom");
    for _ in 0..100 {
        if notifications.recent().unwrap().len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut sent: Vec<(String, String)> =
        notifications.recent().unwrap().into_iter().map(|n| (n.title, n.body)).collect();
    sent.sort();
    assert_eq!(
        sent,
        [
            ("🏁 AI race Cup: round 1".to_string(), "1 of 2 contestants failed this round. The race goes on.".to_string()),
            ("🏁 AI race paused: Cup".to_string(), "It stopped on an error: boom".to_string()),
        ]
    );
}

#[tokio::test]
async fn costs_are_estimated_summed_and_capped() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "priced").await;
    let settings = |input: Option<f64>, output: Option<f64>| ModelProfileSettings {
        input_price_per_million: input,
        output_price_per_million: output,
        ..Default::default()
    };
    let id = service.config(portfolio).unwrap().profile_id.unwrap();
    assert!(service
        .save_profile_with(Some(id), "priced", mock.base_url(), "priced", None, Some(&settings(Some(-1.0), None)))
        .is_err());

    // No prices: the cost stays unknown and the run counts as unpriced.
    mock.script("priced", [Reply::text("Nothing."), Reply::text("{}")]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.cost_usd, None);

    // $2 and $10 per million: each mock reply costs (10*2 + 5*10) / 1e6.
    configure(&service, &mock, portfolio, settings(Some(2.0), Some(10.0)));
    mock.script("priced", [Reply::text("Nothing."), Reply::text("{}")]);
    let run = run_once(&service, portfolio).await;
    let per_reply = (10.0 * 2.0 + 5.0 * 10.0) / 1_000_000.0;
    assert!((run.cost_usd.unwrap() - 2.0 * per_reply).abs() < 1e-12, "{run:?}");
    // A provider that reports no usage leaves the cost unknown.
    mock.script(
        "priced",
        [
            Reply::Ok(json!({"choices": [{"message": {"role": "assistant", "content": "No usage."}}]})),
            Reply::Ok(json!({"choices": [{"message": {"role": "assistant", "content": "{}"}}]})),
        ],
    );
    assert_eq!(run_once(&service, portfolio).await.cost_usd, None);

    let usage = service.usage().unwrap();
    let mine = usage.iter().find(|u| u.kind == "portfolio" && u.name == "priced").unwrap();
    assert!((mine.cost_all - 2.0 * per_reply).abs() < 1e-12, "{mine:?}");
    assert_eq!(mine.cost_7d, mine.cost_all);
    assert_eq!(mine.tokens_all, 4 * 15, "two runs reported usage");
    assert_eq!(mine.unpriced_runs, 2);
    assert!(usage.iter().any(|u| u.kind == "profile" && u.name == "priced"));

    // A run stops once it costs more than its limit.
    let mut config = service.config(portfolio).unwrap();
    assert!(service.save_config(TraderConfig { max_cost_per_run: Some(0.0), ..config.clone() }).is_err());
    config.max_cost_per_run = Some(per_reply * 1.5);
    service.save_config(config.clone()).unwrap();
    mock.script(
        "priced",
        [Reply::tools(&[("get_quote", json!({"ticker": "NVDA"}))]), Reply::tools(&[("get_quote", json!({"ticker": "NVDA"}))])],
    );
    let run = run_once(&service, portfolio).await;
    assert!(error(&run).starts_with("The run reached its cost limit of $0.00"), "{}", error(&run));
    assert!((run.cost_usd.unwrap() - 2.0 * per_reply).abs() < 1e-12, "what it spent is kept");

    // And no run starts once the month's spending reaches its limit.
    config.max_cost_per_run = None;
    config.max_cost_per_month = Some(per_reply);
    service.save_config(config).unwrap();
    let refused = service.start_run(portfolio).await.unwrap_err().to_string();
    assert!(refused.contains("monthly AI cost limit"), "{refused}");
}

#[tokio::test]
async fn testing_a_profile_records_what_the_model_can_do() {
    let service = service();
    let mock = MockModel::start().await;
    let able = trader(&service, &mock, "able").await;
    let able_profile = service.config(able).unwrap().profile_id.unwrap();
    // Two calls at once, arguments sent as an object (normalised to JSON
    // text), then the forced call.
    mock.script(
        "able",
        [
            Reply::message(json!({"role": "assistant", "content": null, "tool_calls": [
                {"id": "a", "type": "function", "function": {"name": "get_price", "arguments": {"ticker": "NVDA"}}},
                {"id": "b", "type": "function", "function": {"name": "get_price", "arguments": "{\"ticker\":\"MSFT\"}"}}
            ]})),
            Reply::tools(&[("get_price", json!({"ticker": "NVDA"}))]),
        ],
    );
    let caps = service.test_profile(able_profile).await.unwrap();
    assert!(caps.tool_calls && caps.json_arguments && caps.parallel_tool_calls && caps.tool_choice, "{caps:?}");
    let stored = service.profiles().unwrap().into_iter().find(|p| p.id == able_profile).unwrap();
    assert_eq!(stored.capabilities.as_ref(), Some(&caps));
    // Renaming keeps the result; another model clears it.
    service.save_profile(Some(able_profile), "Renamed", mock.base_url(), "able", None).unwrap();
    assert!(service.profiles().unwrap().iter().any(|p| p.id == able_profile && p.capabilities.is_some()));
    service.save_profile(Some(able_profile), "Renamed", mock.base_url(), "other-model", None).unwrap();
    assert!(service.profiles().unwrap().iter().any(|p| p.id == able_profile && p.capabilities.is_none()));

    // A model that only talks can't be a trader.
    let chatty = trader(&service, &mock, "chatty").await;
    let chatty_profile = service.config(chatty).unwrap().profile_id.unwrap();
    mock.script("chatty", [Reply::text("NVDA is about $200."), Reply::text("About $200.")]);
    let caps = service.test_profile(chatty_profile).await.unwrap();
    assert!(!caps.tool_calls && !caps.json_arguments && !caps.parallel_tool_calls && !caps.tool_choice, "{caps:?}");
    let refused = service.start_run(chatty).await.unwrap_err().to_string();
    assert!(refused.contains("can't run an AI trader"), "{refused}");
    assert!(service.save_config(service.config(chatty).unwrap()).is_err());
    let other = service
        .create_trader(NewAiTrader {
            name: "Another".into(),
            starting_cash: dec!(1000),
            profile_id: Some(chatty_profile),
            connection: None,
            strategy: "Anything.".into(),
        })
        .await;
    assert!(other.is_err());
}

#[tokio::test]
async fn tool_calls_in_odd_shapes_still_run() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "odd").await;
    // No id, no type, arguments as an object, then as an empty string.
    mock.script(
        "odd",
        [
            Reply::message(json!({"role": "assistant", "tool_calls": [
                {"function": {"name": "get_quote", "arguments": {"ticker": "NVDA"}}},
                {"function": {"name": "get_my_portfolio", "arguments": ""}}
            ]})),
            Reply::text("Looked around."),
            Reply::text("{}"),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert!(run.events.iter().all(|e| e.success), "{:?}", run.events);
    assert!(run.events[0].arguments.contains("NVDA"));
    let sent = &mock.received("odd")[1].body["messages"];
    let assistant = &sent.as_array().unwrap()[2];
    assert_eq!(assistant["tool_calls"][0]["id"], "call_0");
    assert_eq!(assistant["tool_calls"][0]["type"], "function");
    assert!(assistant["tool_calls"][0]["function"]["arguments"].is_string());
}

#[tokio::test]
async fn a_responses_style_profile_runs_a_whole_trade() {
    use dtos::ai_models::ApiStyle;
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "responder").await;
    let id = configure(&service, &mock, portfolio, ModelProfileSettings { api_style: ApiStyle::Responses, ..Default::default() });
    let usage = json!({"input_tokens": 10, "output_tokens": 5, "total_tokens": 15});
    let call = |id: &str, name: &str, args: Value| {
        Reply::Ok(json!({"output": [{"type": "function_call", "call_id": id, "name": name, "arguments": args.to_string()}], "usage": usage}))
    };
    let say = |text: &str| Reply::Ok(json!({"output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": text}]}], "usage": usage}));
    mock.script(
        "responder",
        [
            call("q1", "get_quote", json!({"ticker": "NVDA"})),
            call("o1", "place_order", json!({"ticker": "NVDA", "side": "buy", "shares": 1, "reason": "Responses test."})),
            say("Bought one NVDA."),
            say(r#"{"decision_summary":"Holds NVDA.","unresolved_questions":""}"#),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("Bought one NVDA."));
    assert_eq!(run.total_tokens, Some(60));
    assert!(run.events.iter().any(|e| e.tool == "place_order" && e.success));
    assert_eq!(service.memory(portfolio).unwrap().decision_summary, "Holds NVDA.");

    let sent = mock.received("responder");
    let first = &sent[0].body;
    assert!(first.get("messages").is_none() && first["input"].is_array(), "{first}");
    assert!(first["instructions"].as_str().unwrap().contains("paper-money portfolio"));
    assert_eq!(first["tools"][0]["type"], "function");
    assert!(first["tools"][0]["name"].is_string(), "flat tool definitions");
    // The tool result goes back as a function_call_output for its call.
    let second = sent[1].body["input"].as_array().unwrap();
    assert_eq!(second[second.len() - 2]["type"], "function_call");
    assert_eq!(second.last().unwrap()["type"], "function_call_output");
    assert_eq!(second.last().unwrap()["call_id"], "q1");

    // Testing the connection works the same way.
    mock.script("responder", [call("p1", "get_price", json!({"ticker": "NVDA"})), call("p2", "get_price", json!({"ticker": "NVDA"}))]);
    assert!(service.test_profile(id).await.unwrap().tool_calls);
}

/// Chat Completions stream chunks.
fn delta(content: Value) -> Value {
    json!({"choices": [{"index": 0, "delta": content}]})
}

#[tokio::test]
async fn streamed_replies_are_put_together_and_shown_live() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "streamer").await;
    configure(&service, &mock, portfolio, ModelProfileSettings { stream: true, ..Default::default() });
    let usage = json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}});
    mock.script(
        "streamer",
        [
            // An order whose call arrives in three pieces.
            Reply::stream([
                delta(json!({"role": "assistant", "tool_calls": [{"index": 0, "id": "c1", "type": "function", "function": {"name": "place_order", "arguments": ""}}]})),
                delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "{\"ticker\":\"NVDA\",\"side\":\"buy\","}}]})),
                delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"shares\":1,\"reason\":\"Streamed.\"}"}}]})),
                usage.clone(),
            ]),
            // A slow answer, so the run can be watched mid-way.
            Reply::paced(
                Duration::from_millis(300),
                [delta(json!({"content": "Bought "})), delta(json!({"content": "one NVDA."})), usage.clone()],
            ),
            Reply::stream([delta(json!({"content": "{}"})), usage]),
        ],
    );
    let started = service.start_run(portfolio).await.unwrap();
    let mut seen_live = None;
    for _ in 0..200 {
        let now = service.run(started.id).unwrap();
        if now.status != AiRunStatus::Running {
            break;
        }
        if let Some(text) = now.live_text.filter(|t| t.starts_with("Bought")) {
            seen_live = Some(text);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let run = settle(&service, started).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("Bought one NVDA."));
    assert!(seen_live.is_some(), "the answer was visible while it streamed");
    assert_eq!(run.live_text, None, "nothing live once it's done");
    assert!(run.events.iter().any(|e| e.tool == "place_order" && e.success), "{:?}", run.events);
    assert_eq!(run.total_tokens, Some(45));
    let sent = &mock.received("streamer")[0].body;
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["stream_options"]["include_usage"], true);
}

#[tokio::test]
async fn a_stream_that_goes_quiet_is_retried_and_a_stream_error_fails() {
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "quiet").await;
    configure(&service, &mock, portfolio, ModelProfileSettings { stream: true, timeout_seconds: 1, ..Default::default() });
    // The first stream stops talking for longer than the timeout; the retry
    // is slow overall but never silent for a whole second.
    mock.script(
        "quiet",
        [
            Reply::Stream(vec![
                (Duration::ZERO, delta(json!({"content": "Thinking"})).to_string()),
                (Duration::from_secs(3), delta(json!({"content": " more."})).to_string()),
            ]),
            Reply::paced(
                Duration::from_millis(400),
                [delta(json!({"content": "Slow "})), delta(json!({"content": "but "})), delta(json!({"content": "steady."}))],
            ),
            Reply::stream([delta(json!({"content": "{}"}))]),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("Slow but steady."));
    let retried = retries(&run);
    assert_eq!(retried.len(), 1);
    assert!(retried[0].detail.starts_with("The model went quiet for 1 s mid-answer."), "{}", retried[0].detail);

    mock.script("quiet", [Reply::stream([json!({"error": {"message": "Model overloaded."}})])]);
    let run = run_once(&service, portfolio).await;
    assert_eq!(error(&run), "The model endpoint reported: Model overloaded.");
}

#[tokio::test]
async fn a_responses_style_stream_reads_the_completed_response() {
    use dtos::ai_models::ApiStyle;
    let service = service();
    let mock = MockModel::start().await;
    let portfolio = trader(&service, &mock, "rstream").await;
    configure(&service, &mock, portfolio, ModelProfileSettings { stream: true, api_style: ApiStyle::Responses, ..Default::default() });
    let completed = |output: Value| {
        json!({"type": "response.completed", "response": {"output": output, "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}}})
    };
    mock.script(
        "rstream",
        [
            Reply::stream([
                json!({"type": "response.output_text.delta", "delta": "All "}),
                json!({"type": "response.output_text.delta", "delta": "quiet."}),
                completed(json!([{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "All quiet."}]}])),
            ]),
            Reply::stream([completed(json!([{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "{}"}]}]))]),
        ],
    );
    let run = run_once(&service, portfolio).await;
    assert_eq!(run.status, AiRunStatus::Completed, "{run:?}");
    assert_eq!(run.final_response.as_deref(), Some("All quiet."));
    assert_eq!(run.total_tokens, Some(30));
    let sent = &mock.received("rstream")[0].body;
    assert_eq!(sent["stream"], true);
    assert!(sent.get("stream_options").is_none());
}
