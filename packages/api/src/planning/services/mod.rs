//! Planning use cases: validating and storing targets and goals.

use crate::{planning::repositories::PlanningRepository, shared::ServiceError};
use dtos::planning::{Goal, TargetWeight};
use rust_decimal::Decimal;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
pub struct PlanningService {
    repo: Arc<dyn PlanningRepository>,
}

impl PlanningService {
    pub fn new(repo: Arc<dyn PlanningRepository>) -> Self {
        Self { repo }
    }

    pub fn targets(&self, portfolio_id: Uuid) -> Result<Vec<TargetWeight>, ServiceError> {
        Ok(self.repo.targets(portfolio_id)?)
    }

    /// Weights must each be 0–100, sum to at most 100, with no duplicates.
    /// Zero weights are dropped.
    pub fn set_targets(
        &self,
        portfolio_id: Uuid,
        targets: Vec<TargetWeight>,
    ) -> Result<(), ServiceError> {
        let invalid = |m: String| Err(ServiceError::Validation(m));
        let mut seen = HashSet::new();
        for t in &targets {
            if t.weight < Decimal::ZERO || t.weight > Decimal::ONE_HUNDRED {
                return invalid(format!("{} must be between 0% and 100%", t.ticker));
            }
            if !seen.insert(&t.ticker) {
                return invalid(format!("{} is listed twice", t.ticker));
            }
        }
        let total: Decimal = targets.iter().map(|t| t.weight).sum();
        if total > Decimal::ONE_HUNDRED {
            return invalid(format!(
                "Targets add up to {}%, more than 100%",
                total.normalize()
            ));
        }
        let kept: Vec<TargetWeight> = targets
            .into_iter()
            .filter(|t| t.weight > Decimal::ZERO)
            .collect();
        Ok(self.repo.replace_targets(portfolio_id, &kept)?)
    }

    pub fn goals(&self) -> Result<Vec<Goal>, ServiceError> {
        Ok(self.repo.goals()?)
    }

    pub fn save_goal(&self, mut goal: Goal) -> Result<(), ServiceError> {
        goal.name = goal.name.trim().to_string();
        if goal.name.is_empty() {
            return Err(ServiceError::Validation("Give the goal a name".into()));
        }
        if goal.target <= Decimal::ZERO {
            return Err(ServiceError::Validation(
                "The target must be above zero".into(),
            ));
        }
        if goal.monthly < Decimal::ZERO {
            return Err(ServiceError::Validation(
                "Monthly savings can't be negative".into(),
            ));
        }
        if dtos::planning::days_between("1970-01-01", &goal.date).is_none() {
            return Err(ServiceError::Validation("Date must be YYYY-MM-DD".into()));
        }
        Ok(self.repo.save_goal(&goal)?)
    }

    pub fn delete_goal(&self, id: Uuid) -> Result<(), ServiceError> {
        Ok(self.repo.delete_goal(id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database::Database, planning::infrastructures::SqlitePlanningRepository};
    use rust_decimal_macros::dec;
    use types::ticker_symbol::TickerSymbol;

    fn setup() -> (PlanningService, Uuid) {
        let db = Database::in_memory().unwrap();
        let portfolio = Uuid::new_v4();
        db.with(|c| {
            c.execute(
                "INSERT INTO portfolios (id, name) VALUES (?1, 'P')",
                [portfolio.to_string()],
            )
        })
        .unwrap();
        (
            PlanningService::new(Arc::new(SqlitePlanningRepository::new(db))),
            portfolio,
        )
    }

    fn target(t: &str, w: Decimal) -> TargetWeight {
        TargetWeight {
            ticker: TickerSymbol::new(t).unwrap(),
            weight: w,
        }
    }

    #[test]
    fn targets_validate_and_replace() {
        let (s, p) = setup();
        assert!(matches!(
            s.set_targets(p, vec![target("A", dec!(70)), target("B", dec!(40))]),
            Err(ServiceError::Validation(_))
        ));
        assert!(matches!(
            s.set_targets(p, vec![target("A", dec!(10)), target("A", dec!(10))]),
            Err(ServiceError::Validation(_))
        ));
        s.set_targets(
            p,
            vec![
                target("A", dec!(60)),
                target("B", dec!(40)),
                target("C", dec!(0)),
            ],
        )
        .unwrap();
        let stored = s.targets(p).unwrap();
        assert_eq!(stored.len(), 2, "zero weights dropped");
        assert_eq!(stored[0].weight, dec!(60));
        s.set_targets(p, vec![target("B", dec!(100))]).unwrap();
        assert_eq!(s.targets(p).unwrap().len(), 1, "replaced, not merged");
    }

    #[test]
    fn goals_round_trip() {
        let (s, p) = setup();
        let goal = Goal {
            id: Uuid::new_v4(),
            name: " House ".into(),
            target: dec!(50000),
            date: "2030-01-01".into(),
            monthly: dec!(500),
            portfolio_id: Some(p),
        };
        s.save_goal(goal.clone()).unwrap();
        let stored = &s.goals().unwrap()[0];
        assert_eq!(
            (stored.name.as_str(), stored.portfolio_id),
            ("House", Some(p))
        );
        assert!(matches!(
            s.save_goal(Goal {
                target: dec!(0),
                ..goal.clone()
            }),
            Err(ServiceError::Validation(_))
        ));
        s.delete_goal(goal.id).unwrap();
        assert!(s.goals().unwrap().is_empty());
    }
}
