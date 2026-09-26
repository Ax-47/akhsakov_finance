//! Thesis use cases.

use crate::{shared::ServiceError, thesis::repositories::ThesisRepository};
use dtos::thesis::{
    is_iso_date, Author, Thesis, ThesisDraft, ThesisEntry, MAX_ENTRY_LEN, MAX_EXIT_LEN,
    MAX_THESIS_LEN,
};
use rust_decimal::Decimal;
use std::sync::Arc;
use types::ticker_symbol::TickerSymbol;
use uuid::Uuid;

#[derive(Clone)]
pub struct ThesisService {
    repo: Arc<dyn ThesisRepository>,
}

fn invalid(msg: impl Into<String>) -> ServiceError {
    ServiceError::Validation(msg.into())
}

impl ThesisService {
    pub fn new(repo: Arc<dyn ThesisRepository>) -> Self {
        Self { repo }
    }

    /// Of one portfolio, or every portfolio for `None`.
    pub fn theses(&self, portfolio: Option<Uuid>) -> Result<Vec<Thesis>, ServiceError> {
        Ok(self.repo.theses(portfolio)?)
    }

    pub fn thesis(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Result<Option<Thesis>, ServiceError> {
        Ok(self
            .repo
            .theses(Some(portfolio))?
            .into_iter()
            .find(|t| t.ticker == *ticker))
    }

    /// Checks and saves the thesis fields. A save by the AI also goes in
    /// the journal, with `note` and what changed, so you can see what it
    /// did. Clearing every field of a thesis with no journal deletes it.
    pub fn save(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        draft: ThesisDraft,
        by: Author,
        note: Option<&str>,
    ) -> Result<(), ServiceError> {
        let draft = checked(draft)?;
        let note = note.map(str::trim).filter(|n| !n.is_empty());
        if let Some(note) = note {
            check_entry(note)?;
        }
        let current = self.thesis(portfolio, ticker)?;
        let before = current.as_ref().map(|t| t.draft.clone()).unwrap_or_default();
        if current.is_some() && before == draft && note.is_none() {
            return Ok(());
        }
        if draft == ThesisDraft::default() && note.is_none() && current.as_ref().is_none_or(|t| t.log.is_empty()) {
            return Ok(self.repo.delete(portfolio, ticker)?);
        }
        self.repo.save(portfolio, ticker, &draft, by)?;
        if by == Author::Ai {
            let changes = describe_changes(&before, &draft);
            let text = match (note, changes.is_empty()) {
                (Some(note), true) => note.to_string(),
                (Some(note), false) => format!("{note}\nUpdated: {changes}."),
                (None, false) => format!("Updated: {changes}."),
                (None, true) => return Ok(()),
            };
            self.push_entry(portfolio, ticker, text, by)?;
        } else if let Some(note) = note {
            self.push_entry(portfolio, ticker, note.to_string(), by)?;
        }
        Ok(())
    }

    /// Deletes the thesis and its journal.
    pub fn delete(&self, portfolio: Uuid, ticker: &TickerSymbol) -> Result<(), ServiceError> {
        Ok(self.repo.delete(portfolio, ticker)?)
    }

    /// Adds a dated line to the journal, starting an empty thesis if there
    /// isn't one yet.
    pub fn add_entry(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        text: &str,
        by: Author,
    ) -> Result<ThesisEntry, ServiceError> {
        let text = text.trim();
        check_entry(text)?;
        if self.thesis(portfolio, ticker)?.is_none() {
            self.repo.save(portfolio, ticker, &ThesisDraft::default(), by)?;
        }
        self.push_entry(portfolio, ticker, text.to_string(), by)
    }

    pub fn delete_entry(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_entry(id)?)
    }

    fn push_entry(
        &self,
        portfolio: Uuid,
        ticker: &TickerSymbol,
        text: String,
        by: Author,
    ) -> Result<ThesisEntry, ServiceError> {
        let entry = ThesisEntry {
            id: Uuid::new_v4(),
            author: by,
            text,
            created_at: String::new(),
        };
        self.repo.add_entry(portfolio, ticker, &entry)?;
        Ok(entry)
    }
}

fn check_entry(text: &str) -> Result<(), ServiceError> {
    if text.is_empty() {
        return Err(invalid("Write something first"));
    }
    if text.chars().count() > MAX_ENTRY_LEN {
        return Err(invalid(format!("Journal entries can be at most {MAX_ENTRY_LEN} characters")));
    }
    Ok(())
}

/// The draft trimmed, or why it can't be saved.
fn checked(draft: ThesisDraft) -> Result<ThesisDraft, ServiceError> {
    let thesis = draft.thesis.trim().to_string();
    let exit_if = draft.exit_if.trim().to_string();
    if thesis.chars().count() > MAX_THESIS_LEN {
        return Err(invalid(format!("A thesis can be at most {MAX_THESIS_LEN} characters")));
    }
    if exit_if.chars().count() > MAX_EXIT_LEN {
        return Err(invalid(format!("“Sell if” can be at most {MAX_EXIT_LEN} characters")));
    }
    if draft.target_price.is_some_and(|p| p <= Decimal::ZERO) {
        return Err(invalid("The target price must be above zero"));
    }
    if draft.conviction.is_some_and(|c| !(1..=5).contains(&c)) {
        return Err(invalid("Conviction goes from 1 to 5"));
    }
    let review_on = draft
        .review_on
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());
    if review_on.as_deref().is_some_and(|d| !is_iso_date(d)) {
        return Err(invalid("The review date must be YYYY-MM-DD"));
    }
    Ok(ThesisDraft {
        thesis,
        exit_if,
        target_price: draft.target_price.map(|p| p.normalize()),
        conviction: draft.conviction,
        review_on,
        status: draft.status,
    })
}

/// e.g. "thesis, conviction 4 → 3, status On track → At risk".
fn describe_changes(before: &ThesisDraft, after: &ThesisDraft) -> String {
    fn opt<T: std::fmt::Display>(v: &Option<T>) -> String {
        v.as_ref().map_or("none".into(), |v| v.to_string())
    }
    let mut changes = Vec::new();
    if before.thesis != after.thesis {
        changes.push("thesis".to_string());
    }
    if before.exit_if != after.exit_if {
        changes.push("sell if".to_string());
    }
    if before.target_price != after.target_price {
        changes.push(format!("target {} → {}", opt(&before.target_price), opt(&after.target_price)));
    }
    if before.conviction != after.conviction {
        changes.push(format!("conviction {} → {}", opt(&before.conviction), opt(&after.conviction)));
    }
    if before.review_on != after.review_on {
        changes.push(format!("review date {} → {}", opt(&before.review_on), opt(&after.review_on)));
    }
    if before.status != after.status {
        changes.push(format!("status {} → {}", before.status.label(), after.status.label()));
    }
    changes.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, thesis::infrastructures::SqliteThesisRepository};
    use dtos::thesis::ThesisStatus;
    use rust_decimal_macros::dec;

    fn setup() -> (ThesisService, Uuid, Database) {
        let db = Database::in_memory().unwrap();
        let p = Uuid::new_v4();
        db.with(|c| c.execute("INSERT INTO portfolios (id, name) VALUES (?1, 'Main')", [p.to_string()]))
            .unwrap();
        (ThesisService::new(Arc::new(SqliteThesisRepository::new(db.clone()))), p, db)
    }

    fn sym(s: &str) -> TickerSymbol {
        TickerSymbol::new(s).unwrap()
    }

    fn draft(thesis: &str) -> ThesisDraft {
        ThesisDraft {
            thesis: thesis.into(),
            ..Default::default()
        }
    }

    #[test]
    fn save_checks_and_trims() {
        let (s, p, _) = setup();
        let bad = |d: ThesisDraft| matches!(s.save(p, &sym("NVDA"), d, Author::You, None), Err(ServiceError::Validation(_)));
        assert!(bad(ThesisDraft { conviction: Some(6), ..draft("x") }));
        assert!(bad(ThesisDraft { target_price: Some(dec!(0)), ..draft("x") }));
        assert!(bad(ThesisDraft { review_on: Some("soon".into()), ..draft("x") }));
        assert!(bad(draft(&"x".repeat(MAX_THESIS_LEN + 1))));

        s.save(
            p,
            &sym("NVDA"),
            ThesisDraft {
                exit_if: " Data-centre growth stalls ".into(),
                target_price: Some(dec!(250.00)),
                conviction: Some(4),
                review_on: Some(" ".into()),
                ..draft("  AI capex leader. ")
            },
            Author::You,
            None,
        )
        .unwrap();
        let t = s.thesis(p, &sym("NVDA")).unwrap().unwrap();
        assert_eq!(t.draft.thesis, "AI capex leader.");
        assert_eq!(t.draft.exit_if, "Data-centre growth stalls");
        assert_eq!(t.draft.target_price.unwrap().to_string(), "250");
        assert_eq!(t.draft.review_on, None);
        assert_eq!(t.updated_by, Author::You);
        assert!(t.log.is_empty(), "your own edits aren't journalled");

        assert!(matches!(
            s.save(Uuid::new_v4(), &sym("NVDA"), draft("x"), Author::You, None),
            Err(ServiceError::NotFound(_))
        ));
    }

    #[test]
    fn ai_edits_are_journalled_and_keep_the_journal() {
        let (s, p, _) = setup();
        s.save(p, &sym("AMD"), draft("Share gains in servers"), Author::You, None).unwrap();
        s.add_entry(p, &sym("AMD"), "Q2 beat on data centre", Author::You).unwrap();
        s.save(
            p,
            &sym("AMD"),
            ThesisDraft {
                status: ThesisStatus::AtRisk,
                conviction: Some(3),
                ..draft("Share gains in servers")
            },
            Author::Ai,
            Some("Guidance cut on MI-series delays."),
        )
        .unwrap();
        let t = s.thesis(p, &sym("AMD")).unwrap().unwrap();
        assert_eq!(t.updated_by, Author::Ai);
        assert_eq!(t.log.len(), 2, "an upsert keeps the journal");
        assert_eq!(t.log[0].author, Author::Ai);
        assert_eq!(
            t.log[0].text,
            "Guidance cut on MI-series delays.\nUpdated: conviction none → 3, status On track → At risk."
        );
        assert_eq!(t.log[1].text, "Q2 beat on data centre");

        // An unchanged save by the AI writes nothing.
        s.save(p, &sym("AMD"), t.draft.clone(), Author::Ai, None).unwrap();
        assert_eq!(s.thesis(p, &sym("AMD")).unwrap().unwrap().log.len(), 2);
    }

    #[test]
    fn journal_starts_a_thesis_and_clearing_deletes() {
        let (s, p, db) = setup();
        let entry = s.add_entry(p, &sym("TSM"), " Watching Arizona fab ramp ", Author::Ai).unwrap();
        let t = s.thesis(p, &sym("TSM")).unwrap().unwrap();
        assert_eq!((t.draft.clone(), t.log[0].text.as_str()), (ThesisDraft::default(), "Watching Arizona fab ramp"));
        assert!(matches!(s.add_entry(p, &sym("TSM"), "  ", Author::You), Err(ServiceError::Validation(_))));

        // With a journal, clearing the fields keeps the thesis.
        s.save(p, &sym("TSM"), ThesisDraft::default(), Author::You, None).unwrap();
        assert!(s.thesis(p, &sym("TSM")).unwrap().is_some());
        s.delete_entry(entry.id).unwrap();
        s.save(p, &sym("TSM"), draft("x"), Author::You, None).unwrap();
        s.save(p, &sym("TSM"), ThesisDraft::default(), Author::You, None).unwrap();
        assert!(s.thesis(p, &sym("TSM")).unwrap().is_none());

        // Deleting the portfolio deletes its theses and journals.
        s.add_entry(p, &sym("TSM"), "x", Author::You).unwrap();
        db.with(|c| c.execute("DELETE FROM portfolios", [])).unwrap();
        let left: i64 = db.with(|c| c.query_row("SELECT COUNT(*) FROM thesis_log", [], |r| r.get(0))).unwrap();
        assert_eq!((s.theses(None).unwrap().len(), left), (0, 0));
    }
}
