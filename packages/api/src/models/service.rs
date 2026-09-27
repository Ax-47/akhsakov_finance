use crate::{database::Database, mcp::McpService, shared::ServiceError};
use dtos::ai_models::{AiRun, AiRunEvent, AiRunStatus, ModelProfile, TraderConfig};
use reqwest::Url;
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

const MAX_TURNS: usize = 12;
const MAX_TOOL_CALLS: usize = 24;
const RUN_TIMEOUT: Duration = Duration::from_secs(120);
const ALLOWED_TOOLS: [&str; 5] = [
    "list_theses",
    "get_thesis",
    "get_my_portfolio",
    "get_quote",
    "place_order",
];

#[derive(Clone)]
pub struct ModelService {
    db: Database,
    mcp: McpService,
    http: reqwest::Client,
}

#[derive(Clone)]
struct Connection {
    profile: ModelProfile,
    api_key: String,
}

fn storage(e: impl std::fmt::Display) -> ServiceError {
    ServiceError::Storage(e.to_string())
}

fn invalid(message: impl Into<String>) -> ServiceError {
    ServiceError::Validation(message.into())
}

impl ModelService {
    pub fn new(db: Database, mcp: McpService) -> Self {
        let service = Self {
            db,
            mcp,
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(45))
                .build()
                .expect("build model HTTP client"),
        };
        let _ = service.db.with(|c| {
            c.execute(
                "UPDATE ai_runs SET status = 'failed', finished_at = datetime('now'), error = 'The app stopped before this run finished.' WHERE status = 'running'",
                [],
            )
        });
        service
    }

    pub fn profiles(&self) -> Result<Vec<ModelProfile>, ServiceError> {
        self.db
            .with(|c| {
                c.prepare(
                    "SELECT p.id, p.name, p.base_url, p.model, s.profile_id IS NOT NULL
                     FROM ai_model_profiles p LEFT JOIN ai_model_secrets s ON s.profile_id = p.id
                     ORDER BY lower(p.name)",
                )?
                .query_map([], profile_row)?
                .collect()
            })
            .map_err(storage)
    }

    pub fn save_profile(
        &self,
        id: Option<Uuid>,
        name: &str,
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
    ) -> Result<ModelProfile, ServiceError> {
        let name = name.trim();
        let model = model.trim();
        if name.is_empty() || model.is_empty() {
            return Err(invalid("Profile name and model are required."));
        }
        let base_url = normalize_base_url(base_url)?;
        let id = id.unwrap_or_else(Uuid::new_v4);
        let key = api_key.map(str::trim).filter(|k| !k.is_empty());
        self.db
            .transaction(|tx| {
                tx.execute(
                    "INSERT INTO ai_model_profiles (id, name, base_url, model) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(id) DO UPDATE SET name=excluded.name, base_url=excluded.base_url,
                     model=excluded.model, updated_at=datetime('now')",
                    params![id.to_string(), name, base_url, model],
                )?;
                if let Some(key) = key {
                    tx.execute(
                        "INSERT INTO ai_model_secrets (profile_id, api_key) VALUES (?1, ?2)
                         ON CONFLICT(profile_id) DO UPDATE SET api_key=excluded.api_key",
                        params![id.to_string(), key],
                    )?;
                }
                Ok(())
            })
            .map_err(storage)?;
        self.profile(id)
    }

    pub fn delete_profile(&self, id: Uuid) -> Result<(), ServiceError> {
        let changed = self
            .db
            .with(|c| {
                c.execute(
                    "DELETE FROM ai_model_profiles WHERE id = ?1",
                    [id.to_string()],
                )
            })
            .map_err(storage)?;
        if changed == 0 {
            return Err(ServiceError::NotFound("model profile".into()));
        }
        Ok(())
    }

    pub async fn test_profile(&self, id: Uuid) -> Result<(), ServiceError> {
        let connection = self.connection(id)?;
        let body = json!({
            "model": connection.profile.model,
            "messages": [{"role":"user","content":"Call the connection_test tool once."}],
            "tools": [{"type":"function","function":{
                "name":"connection_test",
                "description":"Confirms tool calling works.",
                "parameters":{"type":"object","properties":{}}
            }}],
            "tool_choice": {"type":"function","function":{"name":"connection_test"}},
            "max_tokens": 32
        });
        let response = self.request(&connection, &body).await?;
        let called = response["choices"][0]["message"]["tool_calls"]
            .as_array()
            .is_some_and(|calls| {
                calls
                    .iter()
                    .any(|c| c["function"]["name"] == "connection_test")
            });
        if !called {
            return Err(invalid(
                "The endpoint answered, but the configured model did not return a tool call.",
            ));
        }
        Ok(())
    }

    pub fn config(&self, portfolio_id: Uuid) -> Result<TraderConfig, ServiceError> {
        self.ensure_ai_portfolio(portfolio_id)?;
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT profile_id, strategy FROM ai_trader_configs WHERE portfolio_id = ?1",
                    [portfolio_id.to_string()],
                    |r| {
                        let profile: Option<String> = r.get(0)?;
                        Ok(TraderConfig {
                            portfolio_id,
                            profile_id: profile.and_then(|s| Uuid::parse_str(&s).ok()),
                            strategy: r.get(1)?,
                        })
                    },
                )
                .optional()
            })
            .map_err(storage)?
            .map_or_else(|| Ok(TraderConfig::new(portfolio_id)), Ok)
    }

    pub fn save_config(&self, config: TraderConfig) -> Result<TraderConfig, ServiceError> {
        self.ensure_ai_portfolio(config.portfolio_id)?;
        if config.strategy.trim().is_empty() {
            return Err(invalid("Trading strategy cannot be empty."));
        }
        if let Some(id) = config.profile_id {
            self.profile(id)?;
        }
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_trader_configs (portfolio_id, profile_id, strategy) VALUES (?1, ?2, ?3)
                     ON CONFLICT(portfolio_id) DO UPDATE SET profile_id=excluded.profile_id, strategy=excluded.strategy",
                    params![
                        config.portfolio_id.to_string(),
                        config.profile_id.map(|id| id.to_string()),
                        config.strategy.trim()
                    ],
                )
            })
            .map_err(storage)?;
        self.config(config.portfolio_id)
    }

    pub async fn start_run(&self, portfolio_id: Uuid) -> Result<AiRun, ServiceError> {
        let config = self.config(portfolio_id)?;
        let profile_id = config
            .profile_id
            .ok_or_else(|| invalid("Choose a model profile first."))?;
        let already_running = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM ai_runs WHERE portfolio_id = ?1 AND status = 'running')",
                    [portfolio_id.to_string()],
                    |r| r.get::<_, bool>(0),
                )
            })
            .map_err(storage)?;
        if already_running {
            return Err(invalid("This portfolio already has a run in progress."));
        }
        let connection = self.connection(profile_id)?;
        let run_id = Uuid::new_v4();
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_runs (id, portfolio_id, status, profile_name, model) VALUES (?1, ?2, 'running', ?3, ?4)",
                    params![
                        run_id.to_string(),
                        portfolio_id.to_string(),
                        connection.profile.name,
                        connection.profile.model
                    ],
                )
            })
            .map_err(|e| {
                let message = e.to_string();
                if message.contains("ai_runs_one_active")
                    || message.contains("UNIQUE constraint failed: ai_runs.portfolio_id")
                {
                    invalid("This portfolio already has a run in progress.")
                } else {
                    storage(e)
                }
            })?;
        let runner = self.clone();
        tokio::spawn(async move {
            let result = tokio::time::timeout(
                RUN_TIMEOUT,
                runner.run_loop(run_id, portfolio_id, &config.strategy, connection),
            )
            .await;
            match result {
                Ok(Ok(done)) => runner.finish(run_id, done, None),
                Ok(Err(e)) => runner.finish(run_id, None, Some(e.to_string())),
                Err(_) => runner.finish(
                    run_id,
                    None,
                    Some("The model run exceeded two minutes.".into()),
                ),
            }
        });
        self.run(run_id)
    }

    pub fn runs(&self, portfolio_id: Uuid) -> Result<Vec<AiRun>, ServiceError> {
        self.ensure_ai_portfolio(portfolio_id)?;
        let ids: Vec<Uuid> = self
            .db
            .with(|c| {
                c.prepare(
                    "SELECT id FROM ai_runs WHERE portfolio_id = ?1 ORDER BY started_at DESC",
                )?
                .query_map([portfolio_id.to_string()], |r| r.get::<_, String>(0))?
                .filter_map(|row| row.ok().and_then(|s| Uuid::parse_str(&s).ok()).map(Ok))
                .collect()
            })
            .map_err(storage)?;
        ids.into_iter().map(|id| self.run(id)).collect()
    }

    pub fn run(&self, id: Uuid) -> Result<AiRun, ServiceError> {
        let mut run = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT portfolio_id, status, profile_name, model, started_at, finished_at,
                            final_response, error, prompt_tokens, completion_tokens, total_tokens
                     FROM ai_runs WHERE id = ?1",
                    [id.to_string()],
                    |r| {
                        let portfolio: String = r.get(0)?;
                        let status: String = r.get(1)?;
                        Ok(AiRun {
                            id,
                            portfolio_id: Uuid::parse_str(&portfolio).unwrap_or_default(),
                            status: parse_status(&status),
                            profile_name: r.get(2)?,
                            model: r.get(3)?,
                            started_at: r.get(4)?,
                            finished_at: r.get(5)?,
                            final_response: r.get(6)?,
                            error: r.get(7)?,
                            prompt_tokens: r.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                            completion_tokens: r.get::<_, Option<i64>>(9)?.map(|v| v as u64),
                            total_tokens: r.get::<_, Option<i64>>(10)?.map(|v| v as u64),
                            events: vec![],
                        })
                    },
                )
                .optional()
            })
            .map_err(storage)?
            .ok_or_else(|| ServiceError::NotFound("AI run".into()))?;
        run.events = self.events(id)?;
        Ok(run)
    }

    async fn run_loop(
        &self,
        run_id: Uuid,
        portfolio_id: Uuid,
        strategy: &str,
        connection: Connection,
    ) -> Result<Option<(String, Usage)>, ServiceError> {
        let portfolio = portfolio_id.to_string();
        let system = format!(
            "You manage one paper-money portfolio in Akhsakov Finance. Your strategy is:\n{strategy}\n\
             Inspect the portfolio and relevant theses before deciding. You may trade immediately with place_order. \
             Every order needs a concise reason. Never claim these paper trades are real or advice to copy. \
             Finish with a concise summary of what you checked, trades made, and why."
        );
        let mut messages = vec![
            json!({"role":"system","content":system}),
            json!({"role":"user","content":"Review this portfolio now and make any paper trades that the strategy calls for."}),
        ];
        let tools = openai_tools();
        let mut tool_count = 0usize;
        let mut usage = Usage::default();
        for _ in 0..MAX_TURNS {
            let response = self
                .request(
                    &connection,
                    &json!({
                        "model": connection.profile.model,
                        "messages": messages,
                        "tools": tools,
                        "tool_choice": "auto",
                        "max_tokens": 2048
                    }),
                )
                .await?;
            usage.add(&response["usage"]);
            let message = response["choices"][0]["message"].clone();
            if !message.is_object() {
                return Err(ServiceError::Upstream(
                    "The model returned no assistant message.".into(),
                ));
            }
            let calls = message["tool_calls"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            messages.push(message.clone());
            if calls.is_empty() {
                let text = message["content"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if text.is_empty() {
                    return Err(ServiceError::Upstream(
                        "The model finished without a response.".into(),
                    ));
                }
                return Ok(Some((text, usage)));
            }
            for call in calls {
                tool_count += 1;
                if tool_count > MAX_TOOL_CALLS {
                    return Err(invalid("The model exceeded the 24 tool-call limit."));
                }
                let call_id = call["id"].as_str().unwrap_or_default();
                let name = call["function"]["name"].as_str().unwrap_or_default();
                if !ALLOWED_TOOLS.contains(&name) {
                    return Err(invalid(format!(
                        "The model requested unavailable tool {name}."
                    )));
                }
                let raw = call["function"]["arguments"].as_str().unwrap_or("{}");
                let mut args: Value = serde_json::from_str(raw).map_err(|_| {
                    invalid(format!("The model returned invalid arguments for {name}."))
                })?;
                if !args.is_object() {
                    args = json!({});
                }
                if matches!(
                    name,
                    "get_my_portfolio" | "list_theses" | "get_thesis" | "place_order"
                ) {
                    args["portfolio"] = json!(portfolio);
                }
                let result = self
                    .mcp
                    .tools()
                    .call(name, &args)
                    .await
                    .flatten_result(name);
                let (content, success) = match result {
                    Ok(value) => (serde_json::to_string(&value).unwrap_or_default(), true),
                    Err(error) => (error, false),
                };
                self.record_event(run_id, tool_count as i64, name, &args, success, &content)?;
                messages.push(json!({
                    "role":"tool",
                    "tool_call_id":call_id,
                    "content":content
                }));
            }
        }
        Err(invalid("The model exceeded the 12-turn limit."))
    }

    async fn request(&self, connection: &Connection, body: &Value) -> Result<Value, ServiceError> {
        let endpoint = format!("{}/chat/completions", connection.profile.base_url);
        let response = self
            .http
            .post(endpoint)
            .bearer_auth(&connection.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| ServiceError::Upstream(format!("Model request failed: {e}")))?;
        let status = response.status();
        let text = response.text().await.map_err(|e| {
            ServiceError::Upstream(format!("Couldn't read the model response: {e}"))
        })?;
        if !status.is_success() {
            let concise: String = text.chars().take(500).collect();
            return Err(ServiceError::Upstream(format!(
                "Model endpoint returned {status}: {concise}"
            )));
        }
        serde_json::from_str(&text)
            .map_err(|_| ServiceError::Upstream("The model endpoint returned invalid JSON.".into()))
    }

    fn finish(&self, id: Uuid, done: Option<(String, Usage)>, error: Option<String>) {
        let (status, response, usage) = match done {
            Some((text, usage)) => ("completed", Some(text), usage),
            None => ("failed", None, Usage::default()),
        };
        let _ = self.db.with(|c| {
            c.execute(
                "UPDATE ai_runs SET status=?2, finished_at=datetime('now'), final_response=?3, error=?4,
                 prompt_tokens=?5, completion_tokens=?6, total_tokens=?7 WHERE id=?1",
                params![
                    id.to_string(), status, response, error,
                    usage.prompt.map(|v| v as i64), usage.completion.map(|v| v as i64), usage.total.map(|v| v as i64)
                ],
            )
        });
    }

    fn record_event(
        &self,
        run_id: Uuid,
        sequence: i64,
        tool: &str,
        args: &Value,
        success: bool,
        result: &str,
    ) -> Result<(), ServiceError> {
        let detail: String = if tool == "place_order" || !success {
            result.chars().take(1_000).collect()
        } else {
            "Completed".into()
        };
        self.db
            .with(|c| {
                c.execute(
                    "INSERT INTO ai_run_events (run_id, sequence, tool, arguments, success, detail)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        run_id.to_string(),
                        sequence,
                        tool,
                        args.to_string(),
                        success,
                        detail
                    ],
                )
            })
            .map_err(storage)?;
        Ok(())
    }

    fn events(&self, id: Uuid) -> Result<Vec<AiRunEvent>, ServiceError> {
        self.db
            .with(|c| {
                c.prepare(
                    "SELECT sequence, tool, arguments, success, detail FROM ai_run_events
                     WHERE run_id = ?1 ORDER BY sequence",
                )?
                .query_map([id.to_string()], |r| {
                    Ok(AiRunEvent {
                        sequence: r.get(0)?,
                        tool: r.get(1)?,
                        arguments: r.get(2)?,
                        success: r.get(3)?,
                        detail: r.get(4)?,
                    })
                })?
                .collect()
            })
            .map_err(storage)
    }

    fn ensure_ai_portfolio(&self, id: Uuid) -> Result<(), ServiceError> {
        self.mcp.trading().book(Some(&id.to_string())).map(|_| ())
    }

    fn profile(&self, id: Uuid) -> Result<ModelProfile, ServiceError> {
        self.db
            .with(|c| {
                c.query_row(
                    "SELECT p.id, p.name, p.base_url, p.model, s.profile_id IS NOT NULL
                     FROM ai_model_profiles p LEFT JOIN ai_model_secrets s ON s.profile_id=p.id WHERE p.id=?1",
                    [id.to_string()],
                    profile_row,
                )
                .optional()
            })
            .map_err(storage)?
            .ok_or_else(|| ServiceError::NotFound("model profile".into()))
    }

    fn connection(&self, id: Uuid) -> Result<Connection, ServiceError> {
        let profile = self.profile(id)?;
        let api_key: Option<String> = self
            .db
            .with(|c| {
                c.query_row(
                    "SELECT api_key FROM ai_model_secrets WHERE profile_id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .optional()
            })
            .map_err(storage)?;
        Ok(Connection {
            profile,
            api_key: api_key.ok_or_else(|| invalid("Enter an API key for this profile."))?,
        })
    }
}

trait ToolResultExt {
    fn flatten_result(self, name: &str) -> Result<Value, String>;
}

impl ToolResultExt for Option<Result<Value, String>> {
    fn flatten_result(self, name: &str) -> Result<Value, String> {
        self.unwrap_or_else(|| Err(format!("Unknown tool: {name}")))
    }
}

#[derive(Default)]
struct Usage {
    prompt: Option<u64>,
    completion: Option<u64>,
    total: Option<u64>,
}

impl Usage {
    fn add(&mut self, value: &Value) {
        add_opt(&mut self.prompt, value["prompt_tokens"].as_u64());
        add_opt(&mut self.completion, value["completion_tokens"].as_u64());
        add_opt(&mut self.total, value["total_tokens"].as_u64());
    }
}

fn add_opt(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or_default() + value);
    }
}

fn normalize_base_url(value: &str) -> Result<String, ServiceError> {
    let value = value.trim().trim_end_matches('/');
    let url =
        Url::parse(value).map_err(|_| invalid("Enter a valid HTTP or HTTPS API base URL."))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(
            "The API base URL must use HTTP or HTTPS and cannot contain credentials.",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(invalid(
            "The API base URL cannot contain a query or fragment.",
        ));
    }
    Ok(value.to_string())
}

fn profile_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ModelProfile> {
    let id: String = r.get(0)?;
    Ok(ModelProfile {
        id: Uuid::parse_str(&id).unwrap_or_default(),
        name: r.get(1)?,
        base_url: r.get(2)?,
        model: r.get(3)?,
        has_key: r.get(4)?,
    })
}

fn parse_status(value: &str) -> AiRunStatus {
    match value {
        "running" => AiRunStatus::Running,
        "completed" => AiRunStatus::Completed,
        _ => AiRunStatus::Failed,
    }
}

fn openai_tools() -> Vec<Value> {
    crate::mcp::services::tools::definitions()
        .as_array()
        .into_iter()
        .flatten()
        .filter(|tool| {
            tool["name"]
                .as_str()
                .is_some_and(|n| ALLOWED_TOOLS.contains(&n))
        })
        .map(|tool| {
            let mut parameters = tool["inputSchema"].clone();
            if let Some(properties) = parameters["properties"].as_object_mut() {
                properties.remove("portfolio");
            }
            json!({
                "type":"function",
                "function": {
                    "name": tool["name"],
                    "description": tool["description"],
                    "parameters": parameters
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (Database, ModelService) {
        let db = Database::in_memory().unwrap();
        let model = ModelService::new(db.clone(), crate::mcp::services::tests::service());
        (db, model)
    }

    #[test]
    fn profile_round_trip_masks_and_replaces_secret() {
        let (_db, service) = service();
        assert!(
            service
                .save_profile(None, "", "https://api.example.com/v1", "model", Some("key"))
                .is_err()
        );
        assert!(
            service
                .save_profile(None, "Example", "file:///tmp/model", "model", Some("key"))
                .is_err()
        );

        let profile = service
            .save_profile(
                None,
                "Example",
                "https://api.example.com/v1/",
                "provider/model",
                Some("secret-one"),
            )
            .unwrap();
        assert_eq!(profile.base_url, "https://api.example.com/v1");
        assert!(profile.has_key);
        assert_eq!(
            service.connection(profile.id).unwrap().api_key,
            "secret-one"
        );

        service
            .save_profile(
                Some(profile.id),
                "Renamed",
                &profile.base_url,
                &profile.model,
                None,
            )
            .unwrap();
        assert_eq!(
            service.connection(profile.id).unwrap().api_key,
            "secret-one"
        );
        service
            .save_profile(
                Some(profile.id),
                "Renamed",
                &profile.base_url,
                &profile.model,
                Some("secret-two"),
            )
            .unwrap();
        assert_eq!(
            service.connection(profile.id).unwrap().api_key,
            "secret-two"
        );
    }

    #[test]
    fn backup_keeps_profile_but_removes_secret() {
        let (db, service) = service();
        service
            .save_profile(
                None,
                "Example",
                "https://api.example.com/v1",
                "model",
                Some("must-not-leave-the-server"),
            )
            .unwrap();
        let bytes = db.export().unwrap();
        assert!(
            !bytes
                .windows("must-not-leave-the-server".len())
                .any(|w| w == b"must-not-leave-the-server")
        );

        let path =
            std::env::temp_dir().join(format!("akhsakov-model-backup-{}.db", Uuid::new_v4()));
        std::fs::write(&path, bytes).unwrap();
        let copy = rusqlite::Connection::open(&path).unwrap();
        let profiles: i64 = copy
            .query_row("SELECT COUNT(*) FROM ai_model_profiles", [], |r| r.get(0))
            .unwrap();
        let secrets: i64 = copy
            .query_row("SELECT COUNT(*) FROM ai_model_secrets", [], |r| r.get(0))
            .unwrap();
        let _ = std::fs::remove_file(path);
        assert_eq!((profiles, secrets), (1, 0));
    }

    #[test]
    fn trader_tool_schema_is_scoped_and_write_limited() {
        let tools = openai_tools();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool["function"]["name"].as_str())
            .collect();
        assert_eq!(names, ALLOWED_TOOLS);
        assert!(!names.contains(&"save_thesis") && !names.contains(&"add_thesis_note"));
        for tool in tools {
            assert!(tool["function"]["parameters"]["properties"]["portfolio"].is_null());
        }
    }
}
