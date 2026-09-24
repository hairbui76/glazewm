use wm_common::WmEvent;

use super::sort_workspaces;
use crate::{
  models::Workspace, user_config::UserConfig, wm_state::WmState,
};

/// Renames the active workspaces so that they hold the leading configured
/// workspace names in order, closing any gaps in the numbering.
///
/// With workspaces `2`, `3`, `5`, `6` and `7` active, for instance, they
/// become `1`, `2`, `3`, `4` and `5`. Windows are not moved: only the
/// workspaces' identity changes, so whichever workspace was focused stays
/// focused and its layout is untouched.
///
/// Workspaces are ordered by monitor and then by their position in the
/// user config, so names stay grouped per monitor.
pub fn rearrange_workspaces(
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let workspaces = ordered_workspaces(state, config);

  let current_names = workspaces
    .iter()
    .map(|workspace| workspace.config().name)
    .collect::<Vec<_>>();

  let configured_names = config
    .value
    .workspaces
    .iter()
    .map(|workspace_config| workspace_config.name.clone())
    .collect::<Vec<_>>();

  let plan = rearrange_plan(&current_names, &configured_names);

  if plan.iter().all(Option::is_none) {
    return Ok(());
  }

  // Resolve the recent workspace up front, since its name is about to
  // change and `general.toggle_workspace_on_refocus` looks it up by name.
  let recent_workspace = state
    .recent_workspace_name
    .clone()
    .and_then(|name| state.workspace_by_name(&name));

  let renamed = workspaces
    .iter()
    .zip(plan)
    .filter_map(|(workspace, config_index)| {
      Some((workspace, config.value.workspaces.get(config_index?)?))
    })
    .collect::<Vec<_>>();

  // Applied in full before any event is emitted, since a workspace can
  // briefly share a name with one that hasn't been renamed yet.
  for (workspace, workspace_config) in &renamed {
    // The whole config is adopted rather than just the name, so that
    // options such as `bind_to_monitor` and `display_name` follow the
    // name instead of the workspace that happened to hold it.
    workspace.set_config((*workspace_config).clone());
  }

  for (workspace, _) in &renamed {
    state.emit_event(WmEvent::WorkspaceUpdated {
      updated_workspace: workspace.to_dto()?,
    });
  }

  state.recent_workspace_name =
    recent_workspace.map(|workspace| workspace.config().name);

  for monitor in state.monitors() {
    sort_workspaces(&monitor, config)?;
  }

  Ok(())
}

/// Active workspaces ordered by monitor, then by their position in the
/// user config.
fn ordered_workspaces(
  state: &WmState,
  config: &UserConfig,
) -> Vec<Workspace> {
  state
    .monitors()
    .iter()
    .flat_map(|monitor| {
      let mut workspaces = monitor.workspaces();
      config.sort_workspaces(&mut workspaces);
      workspaces
    })
    .collect()
}

/// Index of the configured name that each workspace should take, given
/// the workspaces' current names in order.
///
/// An entry is `None` where the workspace already holds the right name,
/// or where there are more workspaces than configured names, in which
/// case the surplus keeps what it has.
fn rearrange_plan(
  current_names: &[String],
  configured_names: &[String],
) -> Vec<Option<usize>> {
  current_names
    .iter()
    .enumerate()
    .map(|(index, current_name)| {
      configured_names
        .get(index)
        .filter(|target_name| *target_name != current_name)
        .map(|_| index)
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::rearrange_plan;

  fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
  }

  #[test]
  fn closes_gaps_in_the_numbering() {
    let configured = names(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    let current = names(&["2", "3", "5", "6", "7"]);

    assert_eq!(
      rearrange_plan(&current, &configured),
      vec![Some(0), Some(1), Some(2), Some(3), Some(4)]
    );
  }

  #[test]
  fn leaves_already_ordered_workspaces_alone() {
    let configured = names(&["1", "2", "3"]);
    let current = names(&["1", "2", "3"]);

    assert_eq!(
      rearrange_plan(&current, &configured),
      vec![None, None, None]
    );
  }

  #[test]
  fn renames_only_the_workspaces_that_are_out_of_place() {
    let configured = names(&["1", "2", "3"]);
    let current = names(&["1", "3"]);

    assert_eq!(rearrange_plan(&current, &configured), vec![None, Some(1)]);
  }

  #[test]
  fn keeps_workspaces_beyond_the_configured_names() {
    let configured = names(&["1"]);
    let current = names(&["2", "3"]);

    assert_eq!(rearrange_plan(&current, &configured), vec![Some(0), None]);
  }

  #[test]
  fn handles_names_that_are_not_numbers() {
    let configured = names(&["web", "code", "chat"]);
    let current = names(&["code", "chat"]);

    assert_eq!(
      rearrange_plan(&current, &configured),
      vec![Some(0), Some(1)]
    );
  }
}
