use crate::application::SearchMatchLocation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DateMeaning {
    #[default]
    SourceEvent,
    ReasoningEvent,
    CollectionActivity,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DateOrder {
    #[default]
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelineRequest {
    #[serde(default)]
    pub date_meaning: DateMeaning,
    #[serde(default)]
    pub order: DateOrder,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub entry_id: String,
    pub page_id: String,
    pub source_id: String,
    pub title: String,
    pub kind: String,
    pub date_label: String,
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub precision: String,
    pub date_status: String,
    pub qualifier: Option<String>,
    pub support: Option<SearchMatchLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimelineResults {
    pub entries: Vec<TimelineEntry>,
    pub next_offset: Option<usize>,
}

pub(crate) struct DateQualification {
    pub label: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub precision: String,
    pub status: String,
}

pub(crate) fn qualify_dates(values: &[&str]) -> DateQualification {
    use chrono::{Datelike, NaiveDate};
    let mut values = values.iter().map(|value| value.trim()).collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    let label = if values.is_empty() {
        "Unknown event date".into()
    } else {
        values.join("; ")
    };
    let mut result = DateQualification {
        label,
        start: None,
        end: None,
        precision: "unknown".into(),
        status: "unknown".into(),
    };
    if values.len() > 1 {
        result.status = "conflicting".into();
        return result;
    }
    let Some(value) = values.first() else {
        return result;
    };
    // Chrono accepts absent whitespace and can split "May 2024" into
    // day 20 and year 24. Check the acquired shape before date conversion.
    let day_shape =
        regex::Regex::new(r"(?i)^(?:[0-9]{4}-[0-9]{2}-[0-9]{2}|[a-z]+\s+[0-9]{1,2},?\s+[0-9]{4})$")
            .expect("built-in date shape");
    if day_shape.is_match(value) {
        for pattern in ["%Y-%m-%d", "%B %e, %Y", "%B %e %Y", "%b %e, %Y", "%b %e %Y"] {
            if let Ok(date) = NaiveDate::parse_from_str(value, pattern) {
                let date = date.format("%Y-%m-%d").to_string();
                result.start = Some(date.clone());
                result.end = Some(date);
                result.precision = "day".into();
                result.status = "known".into();
                return result;
            }
        }
    }
    for pattern in ["%Y-%m-%d", "%B %Y-%d", "%b %Y-%d"] {
        // A supplied month is an interval, never an invented midpoint day.
        let month_input = format!("{value}-01");
        if let Ok(start) = NaiveDate::parse_from_str(&month_input, pattern) {
            let next = if start.month() == 12 {
                NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
            };
            if let Some(end) = next.and_then(|date| date.pred_opt()) {
                result.start = Some(start.format("%Y-%m-%d").to_string());
                result.end = Some(end.format("%Y-%m-%d").to_string());
                result.precision = "month".into();
                result.status = "approximate".into();
                return result;
            }
        }
    }
    result
}
