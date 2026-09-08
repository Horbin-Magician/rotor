use rotor_runtime::QuickAction;

pub(super) enum ActionChange {
    Toggle(String),
    Delete(String),
}

pub(super) struct ActionChangePlan {
    pub actions: Vec<QuickAction>,
    pub save_immediately: bool,
}

/// Only a clean, closed editor can commit a one-click list action. Otherwise
/// preserve every draft and let the user explicitly save the combined changes.
pub(super) fn plan_action_change(
    drafts: &[QuickAction],
    saved: &[QuickAction],
    editing: bool,
    change: ActionChange,
) -> Option<ActionChangePlan> {
    let id = match &change {
        ActionChange::Toggle(id) | ActionChange::Delete(id) => id,
    };
    let index = drafts.iter().position(|action| &action.id == id)?;
    let mut actions = drafts.to_vec();
    match change {
        ActionChange::Toggle(_) => actions[index].enabled = !actions[index].enabled,
        ActionChange::Delete(_) => {
            actions.remove(index);
        }
    }
    Some(ActionChangePlan {
        actions,
        save_immediately: !editing && drafts == saved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn action(id: &str) -> QuickAction {
        QuickAction {
            id: id.into(),
            name: id.into(),
            shortcut: String::new(),
            command: "fixture".into(),
            enabled: false,
        }
    }

    #[test]
    fn clean_list_changes_only_the_selected_saved_action() {
        let saved = vec![action("a"), action("b")];
        let toggle =
            plan_action_change(&saved, &saved, false, ActionChange::Toggle("a".into())).unwrap();
        assert!(toggle.save_immediately);
        assert!(toggle.actions[0].enabled);
        assert_eq!(toggle.actions[1], saved[1]);
        let delete =
            plan_action_change(&saved, &saved, false, ActionChange::Delete("a".into())).unwrap();
        assert!(delete.save_immediately);
        assert_eq!(delete.actions, vec![saved[1].clone()]);
    }

    #[test]
    fn list_actions_never_commit_another_unsaved_command() {
        let saved = vec![action("a"), action("b")];
        let mut drafts = saved.clone();
        drafts[1].command = "unsaved fixture command".into();
        for change in [
            ActionChange::Toggle("a".into()),
            ActionChange::Delete("a".into()),
        ] {
            let plan = plan_action_change(&drafts, &saved, false, change).unwrap();
            assert!(!plan.save_immediately);
            assert_eq!(plan.actions.last().unwrap(), &drafts[1]);
            assert_eq!(saved[1].command, "fixture");
        }
    }

    #[test]
    fn open_editors_new_drafts_and_missing_ids_cannot_trigger_an_implicit_save() {
        let saved = vec![action("a")];
        assert!(
            !plan_action_change(&saved, &saved, true, ActionChange::Toggle("a".into()))
                .unwrap()
                .save_immediately
        );
        let drafts = vec![action("a"), action("new")];
        assert!(
            !plan_action_change(&drafts, &saved, false, ActionChange::Toggle("new".into()))
                .unwrap()
                .save_immediately
        );
        assert!(
            plan_action_change(
                &saved,
                &saved,
                false,
                ActionChange::Delete("missing".into())
            )
            .is_none()
        );
    }
}
