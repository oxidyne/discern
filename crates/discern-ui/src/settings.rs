//! Settings: a shared model (`SettingsModel`) over `settings.json`, and the
//! modal that edits it (`SettingsView`). Everything the UI offers is generated
//! from the registry in `discern-workspace`, so the UI, the JSON file and the
//! validation can never disagree.

use discern_workspace::{DEFINITIONS, Def, Kind, Problem, Settings, SettingsStore, Severity, Val};
use gpui_kit::component::{
    ActiveTheme as _, Sizable, WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{collections::HashMap, rc::Rc};

/// Callback that opens `settings.json` in an editor tab.
pub type OpenJson = Rc<dyn Fn(&mut Window, &mut App)>;

/// Emitted whenever the effective settings may have changed.
#[derive(Debug, Clone, Copy)]
pub struct SettingsChanged;

pub struct SettingsModel {
    store: SettingsStore,
}

impl EventEmitter<SettingsChanged> for SettingsModel {}

pub type SharedSettings = Entity<SettingsModel>;

impl SettingsModel {
    pub fn load(cx: &mut App) -> SharedSettings {
        let path = SettingsStore::default_path()
            .unwrap_or_else(|| std::env::temp_dir().join("discern-settings.json"));
        cx.new(|_| Self { store: SettingsStore::load(path) })
    }

    pub fn settings(&self) -> &Settings { self.store.settings() }
    pub fn problems(&self) -> &[Problem] { self.store.problems() }
    pub fn path(&self) -> &std::path::Path { self.store.path() }

    pub fn set(&mut self, key: &str, value: Val, cx: &mut Context<Self>) -> Result<(), String> {
        self.store.set(key, value).map_err(|e| e.to_string())?;
        cx.emit(SettingsChanged);
        cx.notify();
        Ok(())
    }

    pub fn reset(&mut self, key: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.store.reset(key).map_err(|e| e.to_string())?;
        cx.emit(SettingsChanged);
        cx.notify();
        Ok(())
    }

    pub fn read_text(&self) -> Result<String, String> {
        self.store.read_text().map_err(|e| e.to_string())
    }

    /// Saves hand-edited JSON and re-validates it.
    pub fn write_text(&mut self, text: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.store.write_text(text).map_err(|e| e.to_string())?;
        cx.emit(SettingsChanged);
        cx.notify();
        Ok(())
    }
}

/// Renders a list of problems; shared by the modal and the JSON editor tab.
pub fn problems_banner(problems: &[Problem], cx: &App) -> Option<AnyElement> {
    if problems.is_empty() {
        return None;
    }
    let theme = cx.theme();
    let has_error = problems.iter().any(|p| p.severity == Severity::Error);
    Some(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(if has_error { theme.danger } else { theme.warning })
            .text_sm()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(if has_error {
                "settings.json has problems — invalid entries use their default value"
            } else {
                "settings.json has warnings"
            }))
            .children(problems.iter().take(8).map(|p| div().child(format!("• {p}"))))
            .when(problems.len() > 8, |d| d.child(format!("… and {} more", problems.len() - 8)))
            .into_any_element(),
    )
}

pub struct SettingsView {
    model: SharedSettings,
    inputs: HashMap<&'static str, Entity<InputState>>,
    /// Error from the last edit made through the UI.
    error: Option<String>,
    open_json: OpenJson,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        model: SharedSettings,
        open_json: OpenJson,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut inputs = HashMap::new();
        let mut subscriptions = vec![cx.observe(&model, |_, _, cx| cx.notify())];
        for def in DEFINITIONS.iter().filter(|d| matches!(d.kind, Kind::Int { .. })) {
            let initial = match model.read(cx).settings().get(def.key) {
                Some(Val::Int(i)) => i.to_string(),
                _ => String::new(),
            };
            let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
            let key = def.key;
            subscriptions.push(cx.subscribe_in(&input, window, move |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.commit_number(key, input, window, cx);
                }
            }));
            inputs.insert(def.key, input);
        }
        Self { model, inputs, error: None, open_json, _subscriptions: subscriptions }
    }

    fn commit_number(&mut self, key: &'static str, input: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        let text = input.read(cx).value().trim().to_owned();
        let result = text
            .parse::<i64>()
            .map_err(|_| format!("\"{text}\" is not a whole number"))
            .and_then(|n| self.model.update(cx, |m, cx| m.set(key, Val::Int(n), cx)));
        match result {
            Ok(()) => self.error = None,
            Err(e) => {
                self.error = Some(format!("{key}: {e}"));
                // Show the value actually in effect again.
                self.sync_input(key, window, cx);
            }
        }
        cx.notify();
    }

    fn sync_input(&mut self, key: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(input), Some(Val::Int(i))) = (self.inputs.get(key), self.model.read(cx).settings().get(key).cloned()) {
            input.update(cx, |state, cx| state.set_value(i.to_string(), window, cx));
        }
    }

    fn set(&mut self, key: &'static str, value: Val, cx: &mut Context<Self>) {
        self.error = self.model.update(cx, |m, cx| m.set(key, value, cx)).err();
        cx.notify();
    }

    fn render_control(&self, def: &'static Def, value: &Val, cx: &mut Context<Self>) -> AnyElement {
        match (def.kind, value) {
            (Kind::Bool, Val::Bool(on)) => Switch::new(def.key)
                .checked(*on)
                .on_click(cx.listener(move |this, checked: &bool, _, cx| this.set(def.key, Val::Bool(*checked), cx)))
                .into_any_element(),
            (Kind::Int { min, max }, _) => div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().w(px(90.)).child(Input::new(&self.inputs[def.key])))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("{min}–{max}")))
                .into_any_element(),
            (Kind::Enum(options), Val::Str(current)) => div()
                .flex()
                .gap_1()
                .children(options.iter().enumerate().map(|(i, option)| {
                    let selected = option == current;
                    Button::new((def.key, i))
                        .small()
                        .label(*option)
                        .when(selected, |b| b.primary())
                        .when(!selected, |b| b.ghost())
                        .on_click(cx.listener(move |this, _, _, cx| this.set(def.key, Val::Str((*option).to_owned()), cx)))
                }))
                .into_any_element(),
            _ => div().into_any_element(),
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let settings = model.settings().clone();
        let problems = model.problems().to_vec();
        let path = model.path().display().to_string();
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let danger = cx.theme().danger;

        let mut body = div().flex().flex_col().gap_4();
        let mut category = "";
        for def in DEFINITIONS {
            if def.category != category {
                category = def.category;
                body = body.child(div().pt_2().font_weight(FontWeight::SEMIBOLD).child(category));
            }
            let value = settings.get(def.key).cloned().unwrap_or_else(|| def.default_value());
            let modified = value != def.default_value();
            let control = self.render_control(def, &value, cx);
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .pb_3()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().child(def.title))
                            .child(div().text_xs().text_color(muted).child(def.description))
                            .child(div().text_xs().text_color(muted).child(def.key)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(control)
                            .when(modified, |d| {
                                d.child(Button::new(SharedString::from(format!("reset-{}", def.key))).ghost().xsmall().label("Reset").on_click(
                                    cx.listener(move |this, _, window, cx| {
                                        this.error = this.model.update(cx, |m, cx| m.reset(def.key, cx)).err();
                                        this.sync_input(def.key, window, cx);
                                        cx.notify();
                                    }),
                                ))
                            }),
                    ),
            );
        }

        let open_json = self.open_json.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().min_w_0().truncate().text_xs().text_color(muted).child(path))
                    .child(Button::new("open-settings-json").small().label("Open settings.json").on_click(
                        move |_, window, cx| {
                            window.close_dialog(cx);
                            open_json(window, cx);
                        },
                    )),
            )
            .children(problems_banner(&problems, cx))
            .when_some(self.error.clone(), |d, e| d.child(div().text_sm().text_color(danger).child(e)))
            .child(div().id("settings-scroll").max_h(px(460.)).overflow_y_scroll().child(body))
    }
}

/// Opens the settings modal.
pub fn open_settings_dialog(
    model: SharedSettings,
    open_json: OpenJson,
    window: &mut Window,
    cx: &mut App,
) {
    let view = cx.new(|cx| SettingsView::new(model, open_json, window, cx));
    window.open_dialog(cx, move |dialog, _, _| dialog.title("Settings").w(px(680.)).child(view.clone()));
}
