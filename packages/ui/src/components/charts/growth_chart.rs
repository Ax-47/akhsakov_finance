use dioxus::prelude::*;
use rust_decimal::Decimal;
use std::sync::atomic::{AtomicUsize, Ordering};

static GROWTH_CTR: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: String,
    pub color: String,
    /// One value per chart date; `None` leaves a gap in the line.
    pub values: Vec<Option<Decimal>>,
}

#[component]
pub fn GrowthChart(
    series: ReadSignal<Vec<Series>>,
    chart_dates: ReadSignal<Vec<String>>,
    height: Decimal,
    #[props(default)] title: Option<String>,
) -> Element {
    let chart_id = use_hook(|| {
        format!(
            "echart-growth-{}",
            GROWTH_CTR.fetch_add(1, Ordering::Relaxed)
        )
    });

    let id = chart_id.clone();
    use_effect(move || {
        let series = series.read();
        let grid_top = if title.is_some() { "30px" } else { "10px" };
        let grid_bottom = "28px";

        let series_json = join(series.iter().map(|s| {
            let values = join(s.values.iter().map(|v| match v {
                Some(v) => format!("{v:.4}"),
                None => "null".into(),
            }));
            format!(
                r#"{{"name":{},"color":{},"values":[{values}]}}"#,
                js_str(&s.name),
                js_str(&s.color),
            )
        }));
        let labels_json = join(chart_dates.read().iter().map(|l| js_str(l)));
        let title_json = title.as_deref().map_or("null".into(), js_str);

        let script = format!(
            r#"window.GrowthChart.init("{id}", {{
                labels:     [{labels_json}],
                series:     [{series_json}],
                title:      {title_json},
                gridTop:    "{grid_top}",
                gridBottom: "{grid_bottom}",
            }});"#
        );
        spawn(async move {
            let _ = document::eval(&script).await;
        });
    });

    rsx! {
        div { id: "{chart_id}", style: "width:100%;height:{height}px;" }
    }
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(",")
}

/// Quotes a string as a JS string literal.
fn js_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
