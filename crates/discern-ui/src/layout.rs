//! Panel layout: which panel sits in which dock, in which tab group.
//!
//! The dock's own persistence needs a global panel registry, which cannot
//! build panels that depend on a window's workspace. So the layout is kept as
//! a small description of its own: for each region, its tab groups as lists
//! of stable panel names. It is saved in the workspace session and re-applied
//! at startup when `workbench.restoreLayout` is on.
//!
//! Anything unexpected in a saved layout (unknown or duplicated panels, a
//! missing editor, panels added by a newer version) is repaired by
//! [`normalize`], never trusted.

use gpui_kit::component::dock::{DockArea, DockLayout, DockPlacement, PanelInfo, PanelState, BasePanelView};
use gpui_kit::{App, Context, Window, px};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, sync::Arc};

pub const EDITOR: &str = "discern.editor";
pub const EXPLORER: &str = "discern.explorer";
pub const CONNECTIONS: &str = "discern.connections";
pub const RESULTS: &str = "discern.results";
pub const HISTORY: &str = "discern.history";
pub const INSPECTOR: &str = "discern.inspector";

/// Every panel with the region it belongs to by default.
pub const PANELS: &[(&str, Region)] = &[
    (EDITOR, Region::Center),
    (EXPLORER, Region::Left),
    (CONNECTIONS, Region::Left),
    (RESULTS, Region::Bottom),
    (HISTORY, Region::Bottom),
    (INSPECTOR, Region::Right),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Center,
    Left,
    Right,
    Bottom,
}

impl Region {
    const ALL: [Region; 4] = [Region::Center, Region::Left, Region::Right, Region::Bottom];

    pub fn placement(self) -> DockPlacement {
        match self {
            Region::Center => DockPlacement::Center,
            Region::Left => DockPlacement::Left,
            Region::Right => DockPlacement::Right,
            Region::Bottom => DockPlacement::Bottom,
        }
    }

    fn default_size(self) -> f32 {
        match self {
            Region::Center => 0.,
            Region::Left | Region::Right => 260.,
            Region::Bottom => 280.,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedRegion {
    pub region: Region,
    /// Dock size in pixels (ignored for the center).
    #[serde(default)]
    pub size: f32,
    #[serde(default = "yes")]
    pub open: bool,
    /// Tab groups, each a list of panel names.
    pub groups: Vec<Vec<String>>,
}

fn yes() -> bool { true }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLayout {
    pub regions: Vec<SavedRegion>,
}

impl SavedLayout {
    pub fn default_layout() -> Self {
        let region = |region: Region, groups: &[&[&str]]| SavedRegion {
            region,
            size: region.default_size(),
            open: true,
            groups: groups.iter().map(|g| g.iter().map(|s| (*s).to_owned()).collect()).collect(),
        };
        Self {
            regions: vec![
                region(Region::Center, &[&[EDITOR]]),
                region(Region::Left, &[&[EXPLORER, CONNECTIONS]]),
                region(Region::Right, &[&[INSPECTOR]]),
                region(Region::Bottom, &[&[RESULTS, HISTORY]]),
            ],
        }
    }

    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Makes any saved layout safe to apply: drops unknown and duplicate panels,
/// removes empty groups, appends panels the layout does not mention to their
/// default region, and falls back to the default layout when the editor is
/// missing from the center (the app is unusable without it).
pub fn normalize(saved: SavedLayout) -> SavedLayout {
    let known: HashSet<&str> = PANELS.iter().map(|(n, _)| *n).collect();
    let mut seen = HashSet::new();
    let mut by_region: HashMap<Region, SavedRegion> = HashMap::new();

    for mut r in saved.regions {
        for group in &mut r.groups {
            group.retain(|name| known.contains(name.as_str()) && seen.insert(name.clone()));
        }
        r.groups.retain(|g| !g.is_empty());
        if r.groups.is_empty() {
            continue;
        }
        r.size = if r.size.is_finite() && r.size >= 100. { r.size.min(2000.) } else { r.region.default_size() };
        match by_region.get_mut(&r.region) {
            Some(existing) => existing.groups.extend(r.groups),
            None => { by_region.insert(r.region, r); }
        }
    }

    let editor_in_center = by_region
        .get(&Region::Center)
        .is_some_and(|r| r.groups.iter().flatten().any(|n| n == EDITOR));
    if !editor_in_center {
        return SavedLayout::default_layout();
    }

    for (name, home) in PANELS {
        if seen.contains(*name) {
            continue;
        }
        let region = by_region.entry(*home).or_insert_with(|| SavedRegion {
            region: *home,
            size: home.default_size(),
            open: true,
            groups: Vec::new(),
        });
        match region.groups.first_mut() {
            Some(group) => group.push((*name).to_owned()),
            None => region.groups.push(vec![(*name).to_owned()]),
        }
    }

    SavedLayout {
        regions: Region::ALL.iter().filter_map(|r| by_region.remove(r)).collect(),
    }
}

fn collect_groups(state: &PanelState, out: &mut Vec<Vec<String>>) {
    match &state.info {
        PanelInfo::Tabs { .. } => out.push(state.children.iter().map(|c| c.panel_name.clone()).collect()),
        PanelInfo::Panel(_) => out.push(vec![state.panel_name.clone()]),
        PanelInfo::Stack { .. } => state.children.iter().for_each(|c| collect_groups(c, out)),
    }
}

/// Reads the arrangement currently on screen.
pub fn capture(area: &DockArea, cx: &App) -> SavedLayout {
    let state = area.dump(cx);
    let mut regions = Vec::new();

    let mut groups = Vec::new();
    collect_groups(&state.center, &mut groups);
    regions.push(SavedRegion { region: Region::Center, size: 0., open: true, groups });

    for (region, dock) in [
        (Region::Left, &state.left_dock),
        (Region::Right, &state.right_dock),
        (Region::Bottom, &state.bottom_dock),
    ] {
        if let Some(dock) = dock {
            let mut groups = Vec::new();
            collect_groups(dock.panel(), &mut groups);
            regions.push(SavedRegion { region, size: f32::from(dock.size()), open: dock.open(), groups });
        }
    }
    SavedLayout { regions }
}

/// Installs `layout` (already normalized) into the dock area.
pub fn apply(
    area: &mut DockArea,
    layout: &SavedLayout,
    panels: &HashMap<&'static str, Arc<dyn BasePanelView>>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    for region in &layout.regions {
        let mut dock = DockLayout::v_split();
        for group in &region.groups {
            let mut tabs = DockLayout::tabs();
            for name in group {
                if let Some(panel) = panels.get(name.as_str()) {
                    tabs = tabs.panel_view(panel.clone(), cx);
                }
            }
            dock = dock.child(tabs, None);
        }
        let placement = region.region.placement();
        if region.region == Region::Center {
            area.set_center(dock, window, cx);
        } else {
            area.set_dock(placement, dock, window, cx);
            area.set_dock_size(placement, px(region.size), window, cx);
            area.set_dock_collapsible(placement, true, window, cx);
            if !region.open && area.is_dock_open(placement) {
                area.toggle_dock(placement, window, cx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(regions: Vec<(Region, Vec<Vec<&str>>)>) -> SavedLayout {
        SavedLayout {
            regions: regions
                .into_iter()
                .map(|(region, groups)| SavedRegion {
                    region,
                    size: 300.,
                    open: true,
                    groups: groups.into_iter().map(|g| g.into_iter().map(String::from).collect()).collect(),
                })
                .collect(),
        }
    }

    fn names(l: &SavedLayout, r: Region) -> Vec<String> {
        l.regions.iter().filter(|x| x.region == r).flat_map(|x| x.groups.iter().flatten().cloned()).collect()
    }

    #[test]
    fn default_layout_is_stable_under_normalize() {
        let d = SavedLayout::default_layout();
        assert_eq!(normalize(d.clone()), d);
    }

    #[test]
    fn moved_panel_is_kept_where_the_user_put_it() {
        let l = normalize(layout(vec![
            (Region::Center, vec![vec![EDITOR]]),
            (Region::Right, vec![vec![INSPECTOR, EXPLORER]]),
            (Region::Left, vec![vec![CONNECTIONS]]),
            (Region::Bottom, vec![vec![RESULTS, HISTORY]]),
        ]));
        assert_eq!(names(&l, Region::Right), [INSPECTOR, EXPLORER]);
        assert_eq!(names(&l, Region::Left), [CONNECTIONS]);
    }

    #[test]
    fn unknown_and_duplicate_panels_are_dropped_and_missing_ones_restored() {
        let l = normalize(layout(vec![
            (Region::Center, vec![vec![EDITOR, "ghost.panel", EDITOR]]),
            (Region::Left, vec![vec![EXPLORER]]),
        ]));
        assert_eq!(names(&l, Region::Center), [EDITOR]);
        assert_eq!(names(&l, Region::Left), [EXPLORER, CONNECTIONS]); // connections re-added
        assert_eq!(names(&l, Region::Bottom), [RESULTS, HISTORY]);
        assert_eq!(names(&l, Region::Right), [INSPECTOR]);
    }

    #[test]
    fn missing_editor_falls_back_to_default() {
        let l = normalize(layout(vec![(Region::Left, vec![vec![EDITOR, EXPLORER]])]));
        assert_eq!(l, SavedLayout::default_layout());
        assert_eq!(normalize(SavedLayout { regions: vec![] }), SavedLayout::default_layout());
    }

    #[test]
    fn absurd_sizes_are_repaired() {
        let mut l = layout(vec![(Region::Center, vec![vec![EDITOR]]), (Region::Left, vec![vec![EXPLORER]])]);
        l.regions[1].size = f32::NAN;
        let n = normalize(l);
        assert_eq!(n.regions.iter().find(|r| r.region == Region::Left).unwrap().size, 260.);
    }

    #[test]
    fn garbage_json_is_rejected() {
        assert!(SavedLayout::from_json(&serde_json::json!({"regions": "nope"})).is_none());
        assert!(SavedLayout::from_json(&serde_json::json!(42)).is_none());
    }
}
