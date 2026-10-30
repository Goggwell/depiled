use std::collections::BTreeSet;

use crate::model::Collection;

/// One change to apply to the live client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Create { name: String, apps: Vec<u32> },
    Add { id: String, name: String, apps: Vec<u32> },
    Remove { id: String, name: String, apps: Vec<u32> },
    Rename { id: String, from: String, to: String },
    Delete { id: String, name: String },
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Create { name, apps } => write!(f, "create '{name}' with {} games", apps.len()),
            Op::Add { name, apps, .. } => write!(f, "add {} games to '{name}'", apps.len()),
            Op::Remove { name, apps, .. } => write!(f, "remove {} games from '{name}'", apps.len()),
            Op::Rename { from, to, .. } => write!(f, "rename '{from}' to '{to}'"),
            Op::Delete { name, .. } => write!(f, "delete '{name}'"),
        }
    }
}

/// Changes that turn `current` back into `target`.
///
/// Only static, user-editable collections are considered. Collections that exist now but not
/// in the target are left alone, so undo never deletes something the user made afterwards.
pub fn plan_restore(current: &[Collection], target: &[Collection]) -> Vec<Op> {
    let mut ops = Vec::new();
    for want in target
        .iter()
        .filter(|c| !c.dynamic && Collection::is_user_collection_id(&c.id))
    {
        let found = current
            .iter()
            .find(|c| c.id == want.id)
            .or_else(|| current.iter().find(|c| c.name == want.name));
        let Some(have) = found else {
            ops.push(Op::Create { name: want.name.clone(), apps: want.apps.clone() });
            continue;
        };
        if have.name != want.name {
            ops.push(Op::Rename { id: have.id.clone(), from: have.name.clone(), to: want.name.clone() });
        }
        let have_set: BTreeSet<u32> = have.apps.iter().copied().collect();
        let want_set: BTreeSet<u32> = want.apps.iter().copied().collect();
        let to_add: Vec<u32> = want_set.difference(&have_set).copied().collect();
        let to_remove: Vec<u32> = have_set.difference(&want_set).copied().collect();
        if !to_add.is_empty() {
            ops.push(Op::Add { id: have.id.clone(), name: want.name.clone(), apps: to_add });
        }
        if !to_remove.is_empty() {
            ops.push(Op::Remove { id: have.id.clone(), name: want.name.clone(), apps: to_remove });
        }
    }
    ops
}

/// Like `plan_restore`, but also deletes user collections that exist now and didn't in
/// `target`, so the result matches the snapshot exactly. Used when undoing a change the user
/// just made, where such collections can only have been created since.
pub fn plan_restore_exact(current: &[Collection], target: &[Collection]) -> Vec<Op> {
    let mut ops = plan_restore(current, target);
    for have in current.iter().filter(|c| !c.dynamic && Collection::is_user_collection_id(&c.id)) {
        let kept = target.iter().any(|t| t.id == have.id || t.name == have.name);
        if !kept {
            ops.push(Op::Delete { id: have.id.clone(), name: have.name.clone() });
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_restore_removes_collections_created_since_the_snapshot() {
        let now = vec![col("uc-1", "A", &[1]), col("uc-7", "New", &[9])];
        let before = vec![col("uc-1", "A", &[1])];
        assert_eq!(plan_restore_exact(&now, &before), vec![Op::Delete { id: "uc-7".into(), name: "New".into() }]);
    }

    #[test]
    fn exact_restore_keeps_collections_that_were_only_renamed_in_the_target() {
        let now = vec![col("uc-1", "Renamed", &[1])];
        let before = vec![col("uc-1", "Original", &[1])];
        assert!(!plan_restore_exact(&now, &before).iter().any(|o| matches!(o, Op::Delete { .. })));
    }

    fn col(id: &str, name: &str, apps: &[u32]) -> Collection {
        Collection { id: id.into(), name: name.into(), apps: apps.to_vec(), dynamic: false }
    }

    #[test]
    fn identical_state_needs_no_changes() {
        let a = vec![col("uc-1", "A", &[1, 2])];
        assert!(plan_restore(&a, &a).is_empty());
    }

    #[test]
    fn moves_games_back_and_ignores_order() {
        let now = vec![col("uc-1", "A", &[3, 2]), col("uc-2", "B", &[1])];
        let before = vec![col("uc-1", "A", &[1, 2]), col("uc-2", "B", &[3])];
        let ops = plan_restore(&now, &before);
        assert_eq!(ops.len(), 4);
        assert!(ops.contains(&Op::Add { id: "uc-1".into(), name: "A".into(), apps: vec![1] }));
        assert!(ops.contains(&Op::Remove { id: "uc-1".into(), name: "A".into(), apps: vec![3] }));
    }

    #[test]
    fn recreates_deleted_collection_and_renames_changed_one() {
        let now = vec![col("uc-1", "Renamed", &[1])];
        let before = vec![col("uc-1", "Original", &[1]), col("uc-9", "Gone", &[5, 6])];
        let ops = plan_restore(&now, &before);
        assert_eq!(
            ops,
            vec![
                Op::Rename { id: "uc-1".into(), from: "Renamed".into(), to: "Original".into() },
                Op::Create { name: "Gone".into(), apps: vec![5, 6] },
            ]
        );
    }

    #[test]
    fn never_deletes_collections_created_after_the_snapshot() {
        let now = vec![col("uc-1", "A", &[1]), col("uc-7", "New", &[9])];
        let before = vec![col("uc-1", "A", &[1])];
        assert!(plan_restore(&now, &before).is_empty());
    }

    #[test]
    fn built_in_lists_are_never_recreated() {
        let before = vec![col("favorite", "Favorites", &[]), col("hidden", "Hidden", &[1])];
        assert!(plan_restore(&[], &before).is_empty());
    }

    #[test]
    fn dynamic_collections_are_skipped() {
        let mut dynamic = col("uc-3", "Auto", &[1]);
        dynamic.dynamic = true;
        assert!(plan_restore(&[], &[dynamic]).is_empty());
    }
}
