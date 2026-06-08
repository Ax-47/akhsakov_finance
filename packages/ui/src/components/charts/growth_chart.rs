use dioxus::prelude::*;
use rust_decimal::Decimal;
use std::sync::atomic::{AtomicUsize, Ordering};

static GROWTH_CTR: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub name: String,
    pub color: String,
    pub values: Vec<Decimal>,
}

#[component]
pub fn GrowthChart(
    active_period: Signal<String>,
    series: Vec<Series>,
    chart_dates: Vec<String>,
    height: Decimal,
    #[props(default)] title: Option<String>,
) -> Element {
    let chart_id = use_memo(|| {
        format!(
            "echart-growth-{}",
            GROWTH_CTR.fetch_add(1, Ordering::Relaxed)
        )
    });
    println!("{:?}", series);
    use_effect(move || {
        let _period = active_period.read().clone();

        let id = chart_id.read().clone();
        let show_legend = series.len() > 1;
        let grid_top = if title.is_some() { "30px" } else { "10px" };
        let grid_bottom = if show_legend { "48px" } else { "36px" };

        let series_json = series
            .iter()
            .map(|s| {
                let values = s
                    .values
                    .iter()
                    .map(|v| format!("{v:.4}"))
                    .collect::<Vec<_>>()
                    .join(",");
                let name = s.name.replace('"', "\\\"");
                let color = &s.color;
                format!(r#"{{"name":"{name}","color":"{color}","values":[{values}]}}"#)
            })
            .collect::<Vec<_>>()
            .join(",");

        let labels_json = chart_dates
            .iter()
            .map(|l| format!("\"{}\"", l.replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",");

        let title_json = match title.as_deref() {
            Some(t) => format!("\"{}\"", t.replace('"', "\\\"")),
            None => "null".to_string(),
        };

        let script = format!(
            r#"
            window.GrowthChart.init("{id}", {{
                labels:     [{labels_json}],
                series:     [{series_json}],
                title:      {title_json},
                showLegend: {show_legend},
                gridTop:    "{grid_top}",
                gridBottom: "{grid_bottom}",
            }});
        "#
        );

        spawn(async move {
            let _ = document::eval(&script).await;
        });
    });

    rsx! {
        div { id: "{chart_id}", style: "width:100%;height:{height}px;" }
    }
}
