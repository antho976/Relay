//! Flows through the app with a finger, on sample data, no GPU: what opens what, and that a page
//! under the drawer or a sheet takes no tap (the old app and Leaf both had taps bleed through).

use std::path::PathBuf;
use std::sync::Arc;

use crate::app::{App, Host, Route};
use crate::ui::{Phase, Touch};

struct TestHost(PathBuf);

impl Host for TestHost {
    fn data_dir(&self) -> PathBuf {
        self.0.clone()
    }
    fn log(&self, _text: &str) {}
}

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("relay-code-flow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut a = App::new(Arc::new(TestHost(dir)), 1_000_000_000);
    a.resize(390.0, 844.0, 0.0, 0.0);
    a.sample();
    a
}

fn frames(a: &mut App, clock: &mut u64, n: usize) {
    for _ in 0..n {
        *clock += 16_666_666;
        a.frame(*clock);
    }
}

fn tap(a: &mut App, clock: &mut u64, x: f32, y: f32) {
    *clock += 16_666_666;
    a.touch(Touch { id: 1, phase: Phase::Down, x, y, t_ns: *clock });
    frames(a, clock, 1);
    *clock += 30_000_000;
    a.touch(Touch { id: 1, phase: Phase::Up, x, y, t_ns: *clock });
    frames(a, clock, 30);
}

#[test]
fn the_menu_opens_the_drawer_and_a_row_there_opens_its_chat() {
    let mut a = app("drawer");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 3);
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(a.st.drawer.open, "the menu key opened the drawer");
    // The first row under NEEDS YOU: umber-zebra.
    // The repo, New agent and the search field above the list.
    let list_top = 8.0 + 56.0 + 10.0 + 46.0 + 10.0 + 40.0 + 8.0;
    tap(&mut a, &mut clock, 150.0, list_top + 32.0 + 30.0);
    assert!(!a.st.drawer.open);
    assert_eq!(a.route(), &Route::Chat("umber-zebra".into()));
}

#[test]
fn the_page_under_the_open_drawer_takes_no_tap() {
    let mut a = app("hold");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.drawer.open = true;
    frames(&mut a, &mut clock, 30);
    // Beside the drawer: it closes, and the agent row under that spot does not open.
    tap(&mut a, &mut clock, 360.0, 150.0);
    assert!(!a.st.drawer.open);
    assert_eq!(a.route(), &Route::Home);
}

#[test]
fn the_model_chip_opens_the_model_sheet() {
    let mut a = app("model");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    // The chip sits at the composer's bottom left, after the + key.
    tap(&mut a, &mut clock, 100.0, 844.0 - 10.0 - 10.0 - 17.0);
    assert!(a.st.chat.model_sheet);
    assert!(a.back(), "Back closes the sheet first");
    assert!(!a.st.chat.model_sheet);
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
}

#[test]
fn a_phone_with_no_pc_starts_on_pairing() {
    let dir = std::env::temp_dir().join(format!("relay-code-flow-first-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let a = App::new(Arc::new(TestHost(dir)), 0);
    // The process-wide link may already hold another test's state; the first page follows the store.
    if a.link.profile().is_none() {
        assert_eq!(a.route(), &Route::Pair);
    }
}

#[test]
fn the_repo_sheet_closes_on_a_tap_outside_or_on_its_own_row_and_nothing_under_it_is_hit() {
    let mut a = app("repos");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.drawer.repos = true;
    frames(&mut a, &mut clock, 5);
    // Above the sheet, over the conversation: the sheet closes, the chat stays as it was.
    tap(&mut a, &mut clock, 200.0, 150.0);
    assert!(!a.st.drawer.repos, "a tap outside closed it");
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
    assert!(!a.st.chat.model_sheet && !a.st.drawer.open, "nothing under it took the tap");
    // Open again over the drawer, and tap the repo already chosen: it closes, and the drawer's
    // rows under it do not open an agent.
    a.st.drawer.open = true;
    a.st.drawer.repos = true;
    frames(&mut a, &mut clock, 30);
    let rows = 3.0;
    let top = 844.0 - (70.0 + rows * 56.0 + 12.0);
    tap(&mut a, &mut clock, 150.0, top + 64.0 + 28.0);
    assert!(!a.st.drawer.repos, "its own row closed it");
    assert_eq!(a.repo, Some(1));
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()), "no agent under the sheet was opened");
}

#[test]
fn a_tap_outside_the_model_picker_closes_it_and_reaches_nothing_else() {
    let mut a = app("picker");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.chat.model_sheet = true;
    frames(&mut a, &mut clock, 5);
    // On the header's menu key: under the picker's veil, so the drawer must not open.
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(!a.st.chat.model_sheet);
    assert!(!a.st.drawer.open, "the menu key under the veil took no tap");
}

/// How long a chat frame takes with a long conversation (400 messages, edits with diffs), on
/// this machine's CPU: `cargo test --release chat_frame_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn chat_frame_cost() {
    let mut a = app("bench");
    let long = "The phone matched its own copy to the log by the words, and a queued message is logged differently, so the match missed and both stayed. ".repeat(3);
    let diff = (0..30).map(|i| format!("-    let old_{i} = users.any {{ it.clientId == mine.id }}\n+    let new_{i} = users.any {{ it.id == mine.id && it.text == mine.text }}")).collect::<Vec<_>>().join("\n");
    let mut msgs = Vec::new();
    for i in 0..400 {
        let m = if i % 2 == 0 {
            serde_json::json!({"id": i.to_string(), "role": "user", "created_at": "2026-11-25T10:00:00Z", "body": {"text": format!("Message {i}: push it when the tests pass, and check the board.")}})
        } else {
            serde_json::json!({"id": i.to_string(), "role": "assistant", "created_at": "2026-11-25T10:00:00Z", "body": {"blocks": [
                {"type": "thinking", "text": long},
                {"type": "tool_use", "name": "Edit", "summary": "data/SessionChat.kt", "detail": diff},
                {"type": "text", "text": long},
            ]}})
        };
        msgs.push(m);
    }
    let mut c = crate::data::Convo::default();
    c.take(&serde_json::json!({"cursor": 1, "reset": true, "found": true, "messages": msgs}));
    a.convos.insert("bold-puffin".into(), c);
    a.open("bold-puffin");
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 5);
    let n = 200;
    let t = std::time::Instant::now();
    frames(&mut a, &mut clock, n);
    let per = t.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("chat frame: {per:.0} µs");
    // A frame right after the log changed (a new message came in).
    let t = std::time::Instant::now();
    for _ in 0..20 {
        a.st.chat.forget_layout();
        frames(&mut a, &mut clock, 1);
    }
    println!("chat frame after a new message: {:.0} µs", t.elapsed().as_secs_f64() * 1e6 / 20.0);
}

#[test]
fn a_finger_held_on_a_message_copies_it_and_is_no_tap() {
    struct CopyHost(PathBuf, std::sync::Mutex<Vec<String>>);
    impl Host for CopyHost {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
        fn copy_text(&self, text: &str) {
            self.1.lock().unwrap().push(text.to_string());
        }
    }
    let dir = std::env::temp_dir().join(format!("relay-code-flow-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let host = Arc::new(CopyHost(dir, Default::default()));
    let mut a = App::new(host.clone(), 1_000_000_000);
    a.resize(390.0, 844.0, 0.0, 0.0);
    a.sample();
    a.open("bold-puffin");
    let mut clock = crate::clock::now_ns();
    frames(&mut a, &mut clock, 3);
    // The agent's answer, in the middle of the page (the sample's serif paragraph).
    let (x, y) = (150.0, 300.0);
    a.touch(Touch { id: 1, phase: Phase::Down, x, y, t_ns: clock });
    std::thread::sleep(std::time::Duration::from_millis(500));
    clock = crate::clock::now_ns();
    frames(&mut a, &mut clock, 2);
    a.touch(Touch { id: 1, phase: Phase::Up, x, y, t_ns: clock });
    frames(&mut a, &mut clock, 2);
    // A9: the hold opens a small menu (its time, Copy, Copy as text), and copies nothing yet.
    assert!(a.st.chat.menu.is_some(), "the menu opened");
    assert!(host.1.lock().unwrap().is_empty());
    let [mx, my, mw, _] = crate::chat::menu_rect(&a).unwrap();
    // A tap beside the menu closes it and reaches nothing under it (the fold at the top stays).
    let open_before = a.st.chat.menu;
    tap(&mut a, &mut clock, if mx > 100.0 { 40.0 } else { 370.0 }, my + 20.0);
    assert!(a.st.chat.menu.is_none() && open_before.is_some(), "a tap elsewhere closed it");
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
    // Again, then Copy: one copy.
    a.st.chat.menu = open_before;
    frames(&mut a, &mut clock, 2);
    tap(&mut a, &mut clock, mx + mw / 2.0, my + 36.0 + 22.0);
    let copied = host.1.lock().unwrap().clone();
    assert_eq!(copied.len(), 1, "one copy: {copied:?}");
    assert!(a.st.chat.menu.is_none());
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
}

#[test]
fn the_box_at_homes_foot_opens_new_agent_with_the_keyboard() {
    let mut a = app("start");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 3);
    tap(&mut a, &mut clock, 195.0, 844.0 - 36.0);
    assert!(a.st.newagent.open);
    assert!(a.focus.is("new-agent"));
    assert_eq!(a.focus.kind, crate::kit::Kb::Prose, "a task is written: several lines, corrections");
}

#[test]
fn scrolled_up_a_key_goes_back_to_the_newest() {
    let mut a = app("newest");
    let msgs: Vec<serde_json::Value> = (0..60).map(|i| serde_json::json!({"id": i.to_string(), "role": if i % 2 == 0 { "user" } else { "assistant" }, "created_at": "2026-11-25T10:00:00Z", "body": if i % 2 == 0 { serde_json::json!({"text": format!("message {i}")}) } else { serde_json::json!({"blocks": [{"type": "text", "text": format!("answer {i}, long enough to take a couple of lines on a phone screen at this size.")}]}) }})).collect();
    let mut c = crate::data::Convo::default();
    c.take(&serde_json::json!({"cursor": 1, "reset": true, "found": true, "messages": msgs}));
    a.convos.insert("vivid-ferret".into(), c);
    a.open("vivid-ferret");
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 5);
    // Drag the conversation down a long way: older messages come into view.
    clock += 16_666_666;
    a.touch(Touch { id: 1, phase: Phase::Down, x: 200.0, y: 200.0, t_ns: clock });
    for k in 1..=20 {
        clock += 16_666_666;
        a.touch(Touch { id: 1, phase: Phase::Move, x: 200.0, y: 200.0 + k as f32 * 20.0, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    a.touch(Touch { id: 1, phase: Phase::Up, x: 200.0, y: 600.0, t_ns: clock });
    // Let it coast to a stop: a finger on a coasting list only stops it.
    frames(&mut a, &mut clock, 300);
    assert!(a.st.chat.unstuck(), "scrolled up, it no longer follows");
    // The key sits centred just above the message box.
    let key_y = a.st.chat.box_top - 6.0 - 30.0;
    tap(&mut a, &mut clock, 195.0, key_y);
    assert!(!a.st.chat.unstuck(), "the key took it back to the newest");
}

#[test]
fn effort_in_the_model_picker_is_sent_as_effort() {
    let mut a = app("effort");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    a.st.chat.model_sheet = true;
    frames(&mut a, &mut clock, 2);
    // The card's height: its head, three models, the effort row and its line.
    let card_top = a.st.chat.box_top - 8.0 - (44.0 + 3.0 * 58.0 + 130.0);
    let keys_mid = card_top + 40.0 + 3.0 * 58.0 + 38.0 + 18.0;
    tap(&mut a, &mut clock, 195.0, keys_mid);
    assert!(!a.st.chat.model_sheet, "a level closes the card");
    assert_eq!(a.efforts.get("bold-puffin"), Some(&"high"));
    assert_eq!(a.sending["bold-puffin"].waiting.last().map(|p| p.text.as_str()), Some("/effort high"));
}

#[test]
fn wrapped_lines_cover_every_character_and_keep_an_empty_last_line() {
    let mut a = app("spans");
    let st = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK);
    let text = "one two three four five six seven eight nine ten eleven twelve\nnext line\n";
    let spans = a.canvas.wrap_spans(text, 120.0, st);
    let chars: Vec<char> = text.chars().collect();
    assert!(spans.len() > 3, "it wrapped: {spans:?}");
    assert_eq!(spans.last(), Some(&(chars.len(), chars.len())), "the caret can sit on the empty last line");
    for w in spans.windows(2) {
        assert!(w[0].1 <= w[1].0 && w[1].0 - w[0].1 <= 1, "only a space or a line break between lines: {spans:?}");
    }
    let long = "x".repeat(200);
    let spans = a.canvas.wrap_spans(&long, 120.0, st);
    assert!(spans.len() > 1, "a word wider than the box breaks by characters");
}

#[test]
fn a_long_draft_shows_the_lines_around_the_caret() {
    let mut a = app("draft");
    a.open("bold-puffin");
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 3);
    let draft: String = (1..=40).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
    a.focus.id = Some("chat");
    a.set_text_sel(&draft, None);
    frames(&mut a, &mut clock, 2);
    assert!(a.focus.is("chat"));
    // The caret at the end: the box shows its last line. Then at the start: the first.
    let st = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).line(1.4);
    let fw = 390.0 - 24.0 - 28.0;
    let (first, spans, line) = crate::chat::field_window_for_test(&mut a.canvas, &draft, draft.len(), fw, st);
    assert!(spans.len() > 6 && line == spans.len() - 1 && first == spans.len() - 6);
    let (first, _, line) = crate::chat::field_window_for_test(&mut a.canvas, &draft, 0, fw, st);
    assert_eq!((first, line), (0, 0));
}

#[test]
fn opening_the_drawer_puts_the_keyboard_away() {
    let mut a = app("kb");
    a.open("bold-puffin");
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 3);
    a.focus.id = Some("chat");
    a.st.drawer.open = true;
    frames(&mut a, &mut clock, 2);
    assert!(a.focus.id.is_none(), "typing stopped when the drawer opened");
}

#[test]
fn a_tap_in_the_words_moves_the_caret_there() {
    let mut a = app("caret");
    a.open("bold-puffin");
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 3);
    a.focus.id = Some("chat");
    a.keyboard_up = true;
    a.set_text_sel("hello world", None);
    frames(&mut a, &mut clock, 2);
    assert_eq!(a.focus.selection(), (11, 11));
    let top = a.st.chat.box_top;
    tap(&mut a, &mut clock, 12.0 + 14.0 + 1.0, top + 14.0 + 8.0);
    assert_eq!(a.focus.selection(), (0, 0), "a tap before the first letter puts the caret first");
    assert!(a.focus.is("chat"));
}

// D1: agents you manage.

#[test]
fn an_earlier_agent_on_home_opens_its_chat_read_only() {
    let mut a = app("earlier");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 3);
    // Under NEEDS YOU (one row) and AGENTS (three), EARLIER's first row: keen-newt, stopped.
    let row = crate::drawer::AGENT_ROW_H;
    let earlier = 56.0 + 4.0 + (34.0 + row + 8.0) + (34.0 + 3.0 * row + 8.0) + 32.0;
    tap(&mut a, &mut clock, 150.0, earlier + 30.0);
    assert_eq!(a.route(), &Route::Chat("keen-newt".into()));
    // No message box: a tap where it would be gives no keyboard.
    tap(&mut a, &mut clock, 120.0, 844.0 - 38.0);
    assert!(a.focus.id.is_none(), "a stopped agent's chat takes no message");
}

#[test]
fn the_chat_menu_opens_from_its_key_and_a_tap_outside_it_reaches_nothing() {
    let mut a = app("agent-menu");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    tap(&mut a, &mut clock, 390.0 - 50.0 + 22.0, 8.0 + 1.0 + 22.0);
    assert_eq!(a.st.agents.menu.as_deref(), Some("bold-puffin"));
    // On the header's drawer key, under the menu: it closes, and the drawer stays shut.
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(a.st.agents.menu.is_none());
    assert!(!a.st.drawer.open, "the key under the menu took no tap");
}

#[test]
fn close_asks_first_and_a_tap_outside_the_dialog_reaches_nothing() {
    let mut a = app("close");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.agents.menu = Some("bold-puffin".into());
    frames(&mut a, &mut clock, 5);
    // The menu's fourth row: Close agent (after Find in chat, Open terminal and Pause agent).
    tap(&mut a, &mut clock, 280.0, 52.0 + 4.0 + 3.0 * 48.0 + 24.0);
    assert!(a.st.agents.menu.is_none());
    assert_eq!(a.st.agents.confirm, Some(("bold-puffin".into(), crate::agents::Confirm::Close)));
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(a.st.agents.confirm.is_none(), "a tap outside cancels");
    assert!(!a.st.drawer.open, "and reaches nothing under the dialog");
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
    // Back closes the menu as it closes any sheet.
    a.st.agents.menu = Some("bold-puffin".into());
    assert!(a.back());
    assert!(a.st.agents.menu.is_none());
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
}

#[test]
fn the_drawer_search_keeps_its_keyboard_and_finds_by_intent() {
    let mut a = app("search");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.drawer.open = true;
    frames(&mut a, &mut clock, 30);
    let search_y = 8.0 + 56.0 + 10.0 + 46.0 + 10.0;
    tap(&mut a, &mut clock, 150.0, search_y + 20.0);
    assert_eq!(a.focus.id, Some(crate::agents::SEARCH));
    a.set_text("LEAF");
    frames(&mut a, &mut clock, 3);
    assert_eq!(a.focus.id, Some(crate::agents::SEARCH), "the open drawer keeps its own field");
    let found = crate::agents::found(&a, "LEAF");
    assert_eq!(found.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["swift-ocelot", "vivid-ferret"]);
    // The first found row opens it.
    let list_top = search_y + 40.0 + 8.0;
    tap(&mut a, &mut clock, 150.0, list_top + 32.0 + 30.0);
    assert_eq!(a.route(), &Route::Chat("swift-ocelot".into()));
    assert!(!a.st.drawer.open);
}

// D2: starting an agent.

/// New agent's sheet on a 390 × 844 screen with no insets: its top, and the middle of each row.
struct Sheet {
    model_y: f32,
    effort_y: f32,
    task_y: f32,
}

fn new_agent_sheet() -> Sheet {
    let top = 844.0 - (82.0 + 96.0 + 16.0 + 2.0 * 46.0 + 6.0 + 48.0 + 14.0 + 48.0 + 16.0);
    // The rows' tops: model and effort (36 each, 10 apart), then the task row (48) after 6 more.
    let rows = top + 82.0 + 96.0 + 16.0;
    Sheet { model_y: rows + 18.0, effort_y: rows + 46.0 + 18.0, task_y: rows + 2.0 * 46.0 + 6.0 + 24.0 }
}

#[test]
fn new_agent_remembers_the_model_and_effort_last_chosen() {
    let mut a = app("choice");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.newagent.open = true;
    frames(&mut a, &mut clock, 5);
    let s = new_agent_sheet();
    // The tracks start after a 64-unit label: Opus is the second of four, High the fourth of six.
    let (x0, w) = (16.0 + 64.0, 390.0 - 32.0 - 64.0);
    tap(&mut a, &mut clock, x0 + w / 4.0 * 1.5, s.model_y);
    tap(&mut a, &mut clock, x0 + w / 6.0 * 3.5, s.effort_y);
    assert_eq!(a.last_choice(), (Some("opus".into()), Some("high".into())));
    assert!(a.st.newagent.open, "choosing keeps the sheet up");
    assert_eq!(a.route(), &Route::Home, "and reaches nothing under it");
    // Auto goes back to Claude Code's own default.
    tap(&mut a, &mut clock, x0 + w / 4.0 * 0.5, s.model_y);
    assert_eq!(a.last_choice(), (None, Some("high".into())));
}

#[test]
fn the_task_list_opens_over_new_agent_and_back_returns_to_it() {
    let mut a = app("tasks");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.newagent.open = true;
    frames(&mut a, &mut clock, 5);
    tap(&mut a, &mut clock, 150.0, new_agent_sheet().task_y);
    assert!(a.st.newagent.picking, "the task row opens the list");
    a.st.newagent.tasks = a.st.board.tasks.clone();
    frames(&mut a, &mut clock, 3);
    // Back closes the list, not New agent.
    assert!(a.back());
    assert!(!a.st.newagent.picking && a.st.newagent.open);
    // Again, and pick the first open task (Active first): it is linked, and the list goes.
    a.st.newagent.picking = true;
    frames(&mut a, &mut clock, 3);
    let rows = a.st.newagent.tasks.iter().filter(|t| t.column != "done").count().clamp(1, 8) as f32;
    let top = 844.0 - (70.0 + rows * 56.0 + 12.0);
    tap(&mut a, &mut clock, 150.0, top + 64.0 + 28.0);
    assert!(!a.st.newagent.picking);
    assert_eq!(a.st.newagent.linked.as_ref().map(|l| l.0), Some(41));
    // A tap above the list closes it, and reaches no agent row under it.
    a.st.newagent.picking = true;
    frames(&mut a, &mut clock, 3);
    tap(&mut a, &mut clock, 150.0, 120.0);
    assert!(!a.st.newagent.picking);
    assert_eq!(a.route(), &Route::Home);
}

#[test]
fn a_made_agent_opens_its_chat_at_once_starting() {
    let mut a = app("created");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.newagent.open = true;
    a.st.newagent.task = "Add a dark mode toggle".into();
    a.st.newagent.linked = Some((41, "Dark mode".into()));
    a.created(&serde_json::json!({"name": "brisk-lynx", "project_id": 1, "provider": "claude", "role": "builder", "state": "created", "branch": "relay/brisk-lynx", "model": "opus", "effort": "high"}));
    frames(&mut a, &mut clock, 3);
    assert_eq!(a.route(), &Route::Chat("brisk-lynx".into()));
    assert!(!a.st.newagent.open && a.st.newagent.task.is_empty() && a.st.newagent.linked.is_none());
    let made = a.agent("brisk-lynx").expect("it is listed before the PC lists it");
    assert_eq!(made.state_words(), "starting");
    assert!(a.busy("brisk-lynx"), "its chat shows it starting");
}

// D5: the look pass.

#[test]
fn the_drawers_list_scrolls_to_earlier_under_its_links() {
    let mut a = app("drawer-scroll");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.drawer.open = true;
    frames(&mut a, &mut clock, 30);
    assert!(a.st.drawer.scroll.max > 0.0, "live and earlier agents overflow the drawer's list");
    // A drag up the list moves it, not the page under the drawer.
    let (x, from) = (150.0, 400.0);
    clock += 16_666_666;
    a.touch(Touch { id: 1, phase: Phase::Down, x, y: from, t_ns: clock });
    frames(&mut a, &mut clock, 1);
    for i in 1..=10 {
        clock += 16_666_666;
        a.touch(Touch { id: 1, phase: Phase::Move, x, y: from - 20.0 * i as f32, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    clock += 100_000_000;
    a.touch(Touch { id: 1, phase: Phase::Up, x, y: from - 200.0, t_ns: clock });
    frames(&mut a, &mut clock, 30);
    assert!(a.st.drawer.scroll.off > 100.0, "the list scrolled: {}", a.st.drawer.scroll.off);
    assert!(a.st.drawer.open);
    assert_eq!(a.route(), &Route::Home);
}

// D6: Settings.

/// Settings' rows on a 390 × 844 screen with sample data (no profile, so no version line).
const NOTIFY_TOP: f32 = 56.0 + 52.0 + 30.0 + 36.0 + 24.0 + 22.0 + 30.0;
/// Under the switches: the text size row (its label, the choices and their room), then the app.
const TEXT_TOP: f32 = NOTIFY_TOP + 3.0 * 52.0 + 22.0;
/// About: its label, the build's row, Check for updates now, then Forget.
const ABOUT_TOP: f32 = TEXT_TOP + 30.0 + 36.0 + 8.0 + 22.0;
const CHECK_TOP: f32 = ABOUT_TOP + 30.0 + 48.0;
const FORGET_TOP: f32 = CHECK_TOP + 52.0 + 12.0;

#[test]
fn a_notification_row_switches_it_and_the_choice_is_kept() {
    use crate::settings::Notify;
    let mut a = app("notify");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    frames(&mut a, &mut clock, 3);
    assert!(a.notifies(Notify::Done), "on until switched off");
    tap(&mut a, &mut clock, 200.0, NOTIFY_TOP + 52.0 + 26.0);
    assert!(!a.notifies(Notify::Done));
    assert!(a.notifies(Notify::Asks) && a.notifies(Notify::Holds), "only that row");
    // Kept in the app's config, where the background loop reads it without an App.
    let dir = a.data.clone();
    let kept = (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(10));
        !crate::app::notifies_in(&dir, Notify::Done)
    });
    assert!(kept, "written to config.json");
    assert!(crate::app::notifies_in(&dir, Notify::Asks));
    tap(&mut a, &mut clock, 200.0, NOTIFY_TOP + 52.0 + 26.0);
    assert!(a.notifies(Notify::Done), "and on again");
}

#[test]
fn forget_this_pc_asks_first_and_a_tap_outside_keeps_it() {
    let mut a = app("forget");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    frames(&mut a, &mut clock, 3);
    tap(&mut a, &mut clock, 200.0, FORGET_TOP + 23.0);
    assert!(crate::settings::confirming(&a));
    // On the Back key, outside the dialog: it closes, and Back under it is not pressed.
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(!crate::settings::confirming(&a));
    assert_eq!(a.route(), &Route::Settings, "nothing under the dialog took the tap");
}

#[test]
fn the_rows_say_working_when_the_terminal_does_as_the_header_does() {
    let mut a = app("words");
    let ocelot = a.agent("swift-ocelot").cloned().expect("sample agent");
    // Its output is a minute old, so the list alone reads it as ready.
    assert_eq!(a.agent_words(&ocelot), "ready");
    a.put_screen("swift-ocelot", crate::screen::read("· Pondering… (3s · thought for 1s)\n────\n❯ \n────\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"));
    assert_eq!(a.agent_words(&ocelot), "working");
    // A11: read long ago for a chat no longer open, it no longer says so: the list does.
    a.now_ns += 11_000_000_000;
    assert_eq!(a.agent_words(&ocelot), "ready", "a stale foot is not trusted");
    // One that asks says so, whatever its terminal shows.
    let zebra = a.agent("umber-zebra").cloned().expect("sample agent");
    assert_eq!(a.agent_words(&zebra), "asks you");
}

// D3: commands that fit the repo.

#[test]
fn a_bang_command_you_sent_heads_that_agents_list_and_no_other() {
    let mut a = app("bang");
    a.open("swift-ocelot");
    a.say("swift-ocelot", "!cargo test -p relaycode flows");
    let l = crate::commands::list(&mut a, "swift-ocelot", "!");
    assert_eq!(l[0].text, "!cargo test -p relaycode flows");
    assert_eq!(l[0].what, "You ran this");
    let other = crate::commands::list(&mut a, "umber-zebra", "!");
    assert!(other.iter().all(|c| c.text != "!cargo test -p relaycode flows"));
    // Words to the agent are not commands.
    a.say("swift-ocelot", "Run the tests please");
    assert_eq!(crate::commands::list(&mut a, "swift-ocelot", "!")[0].text, "!cargo test -p relaycode flows");
    assert!(crate::commands::list(&mut a, "swift-ocelot", "Run").is_empty());
}

// D8: first run.

/// Where the pairing screen's keys start, below its mark, title and lead (no insets).
fn pair_keys_top(a: &mut App) -> f32 {
    use crate::theme::ty;
    let lead = "Once, here. After that this phone connects to your PC by itself, every time.";
    let lead_h = a.canvas.para_h(lead, 390.0 - 48.0, crate::draw::Text::new(ty::UI, crate::theme::INK2).line(1.45), 3);
    48.0 + 48.0 + 36.0 + lead_h + 28.0
}

#[test]
fn pairing_offers_the_link_way_behind_a_quiet_key_and_comes_back() {
    let mut a = app("pair-ways");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Pair);
    frames(&mut a, &mut clock, 3);
    // The server field (its label and box), the key, then the quiet key.
    let field_h = crate::draw::Text::new(crate::theme::ty::CAPTION, crate::theme::INK2).lh() + 6.0 + 48.0 + 14.0;
    let quiet = pair_keys_top(&mut a) + field_h + 48.0 + 8.0 + 22.0;
    tap(&mut a, &mut clock, 195.0, quiet);
    assert!(a.st.pair.by_link, "the quiet key shows the link way");
    // The link way has the same shape: its field, its key, and the way back.
    tap(&mut a, &mut clock, 195.0, quiet);
    assert!(!a.st.pair.by_link, "and its own quiet key goes back");
    assert_eq!(a.route(), &Route::Pair);
}

#[test]
fn cancel_leaves_the_wait_for_discord() {
    let mut a = app("pair-cancel");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Pair);
    crate::pair::sample_waiting(&mut a, false);
    frames(&mut a, &mut clock, 3);
    assert!(a.st.pair.error.is_none());
    // Under the code, its two steps (one line and two lines at 390 wide), then Cancel.
    let steps_top = pair_keys_top(&mut a) + 22.0 + 64.0;
    let step_lh = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).line(1.4).lh();
    let cancel = steps_top + (2.0 * step_lh + 14.0) + (step_lh + 14.0) + 16.0 + 23.0;
    tap(&mut a, &mut clock, 195.0, cancel);
    assert!(!crate::pair::waiting(&a), "Cancel stops waiting");
}

// D7: no tap falls through an overlay.

#[test]
fn a_command_in_the_list_fills_the_box_and_nothing_under_it_is_hit() {
    let mut a = app("palette");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    a.focus.id = Some("chat");
    a.focus.text = "/".into();
    frames(&mut a, &mut clock, 3);
    let n = crate::commands::list(&mut a, "bold-puffin", "/").len() as f32;
    // The list sits on the message box: its first row is the top one.
    let top = a.st.chat.box_top - (n * 42.0 + 12.0) - 6.0;
    let first = crate::commands::list(&mut a, "bold-puffin", "/")[0].text.clone();
    tap(&mut a, &mut clock, 120.0, top + 6.0 + 21.0);
    assert_eq!(a.focus.text, format!("{first} "), "the row filled the box");
    assert!(a.sending.get("bold-puffin").is_none_or(|s| s.waiting.iter().all(|p| !p.text.starts_with('/'))), "and sent nothing");
    assert!(!a.st.chat.model_sheet && !a.st.drawer.open && a.st.agents.menu.is_none(), "nothing under the list took it");
}

#[test]
fn a_tap_above_new_agent_closes_it_and_opens_no_agent_under_it() {
    let mut a = app("new-outside");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.st.newagent.open = true;
    a.st.newagent.task = "Kept for next time".into();
    frames(&mut a, &mut clock, 5);
    // Home's NEEDS YOU row (umber-zebra) is under this spot, above the sheet.
    tap(&mut a, &mut clock, 150.0, 56.0 + 4.0 + 34.0 + 30.0);
    assert!(!a.st.newagent.open, "a tap outside closes it");
    assert_eq!(a.route(), &Route::Home, "and the row under it does not open");
    assert_eq!(a.st.newagent.task, "Kept for next time", "what was typed waits for next time");
}

#[test]
fn a_stopped_agents_foot_takes_its_own_taps() {
    let mut a = app("stopped-foot");
    let mut clock = 1_000_000_000;
    a.open("keen-newt");
    frames(&mut a, &mut clock, 5);
    // The foot's words, over the end of the conversation: no copy, no keyboard, no page change.
    tap(&mut a, &mut clock, 80.0, 844.0 - crate::agents::STOPPED_H / 2.0);
    assert!(a.focus.id.is_none());
    assert_eq!(a.route(), &Route::Chat("keen-newt".into()));
    assert!(a.st.agents.menu.is_none() && a.st.agents.confirm.is_none());
}

// D10: updates from the PC.

fn update_sheet_keys_y(a: &mut App) -> f32 {
    use crate::theme::ty;
    let w = 390.0 - 2.0 * (crate::theme::PAD + 4.0);
    let line = format!("{}. It downloads from your PC, then Android asks to install it.", if crate::update::BUILD == 0 { "This build has no number".to_string() } else { format!("You have {}", crate::update::version_of(crate::update::BUILD)) });
    let note_h = a.canvas.para_h(&line, w, crate::draw::Text::new(ty::UI, crate::theme::INK3).line(1.4), 4);
    let h = 30.0 + 26.0 + 10.0 + note_h + 22.0 + 48.0 + 18.0;
    844.0 - h + 30.0 + 26.0 + 10.0 + note_h + 22.0 + 24.0
}

#[test]
fn a_newer_build_on_the_pc_is_offered_and_later_hides_it_for_a_day() {
    let mut a = app("update-later");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    let l = |build| crate::update::Latest { build, version: format!("0.{build}"), size: 18_400_000, sha256: "00".into(), made_at: 0 };
    // Not newer than this app: nothing.
    crate::update::offered(&mut a, l(crate::update::BUILD));
    assert!(!a.st.update.open);
    crate::update::offered(&mut a, l(crate::update::BUILD + 5));
    assert!(a.st.update.open, "a newer build opens the sheet");
    frames(&mut a, &mut clock, 5);
    // Later, the left key.
    let y = update_sheet_keys_y(&mut a);
    tap(&mut a, &mut clock, 80.0, y);
    assert!(!a.st.update.open);
    assert_eq!(a.update_later().map(|l| l.0), Some(crate::update::BUILD + 5));
    // The same build again (the next check): still put off.
    a.st.update.latest = None;
    crate::update::offered(&mut a, l(crate::update::BUILD + 5));
    assert!(!a.st.update.open, "put off for a day");
    // A newer one still comes.
    crate::update::offered(&mut a, l(crate::update::BUILD + 6));
    assert!(a.st.update.open);
}

#[test]
fn a_tap_above_the_update_sheet_closes_it_and_reaches_nothing_and_update_says_where_it_installs() {
    let mut a = app("update-outside");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    crate::update::sample(&mut a, crate::update::BUILD + 1, None);
    frames(&mut a, &mut clock, 5);
    // Home's NEEDS YOU row is under this spot.
    tap(&mut a, &mut clock, 150.0, 56.0 + 4.0 + 34.0 + 30.0);
    assert!(!a.st.update.open);
    assert_eq!(a.route(), &Route::Home, "the row under the veil did not open");
    // On a host that cannot install (tests, the desktop), Update says so and downloads nothing.
    a.st.update.open = true;
    frames(&mut a, &mut clock, 5);
    let y = update_sheet_keys_y(&mut a);
    tap(&mut a, &mut clock, 300.0, y);
    assert!(!a.st.update.open && !a.st.update.downloading);
    // Back closes the sheet as it closes any.
    a.st.update.open = true;
    assert!(a.back());
    assert!(!a.st.update.open);
}

// D4: notifications land right; Back closes what is over the page first.

// What a tap hands over goes through background::take_opened, a process-wide slot every app's
// frame reads; tests call what the frame does with it, so a parallel test's frames cannot take it.

#[test]
fn a_tapped_notification_opens_its_chat_over_whatever_was_open() {
    let mut a = app("notified");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.drawer.open = true;
    a.st.drawer.repos = true;
    a.st.chat.model_sheet = true;
    frames(&mut a, &mut clock, 5);
    a.open_notified("umber-zebra");
    frames(&mut a, &mut clock, 3);
    assert_eq!(a.route(), &Route::Chat("umber-zebra".into()));
    assert!(!a.st.drawer.open && !a.st.drawer.repos && !a.st.chat.model_sheet, "nothing left over it");
    // Back from it goes to Home, as from any chat.
    assert!(a.back());
    assert_eq!(a.route(), &Route::Home);
}

#[test]
fn a_notification_while_pairing_opens_nothing() {
    let mut a = app("notified-pair");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Pair);
    a.open_notified("umber-zebra");
    frames(&mut a, &mut clock, 3);
    assert_eq!(a.route(), &Route::Pair);
}

#[test]
fn back_closes_the_sheet_then_the_drawer_then_the_page() {
    let mut a = app("back-order");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.drawer.open = true;
    a.st.drawer.repos = true;
    frames(&mut a, &mut clock, 5);
    assert!(a.back());
    assert!(!a.st.drawer.repos && a.st.drawer.open, "the repo sheet first");
    assert!(a.back());
    assert!(!a.st.drawer.open, "then the drawer");
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
    // A menu over the chat, then the chat.
    a.st.agents.menu = Some("bold-puffin".into());
    assert!(a.back());
    assert!(a.st.agents.menu.is_none());
    assert!(a.back());
    assert_eq!(a.route(), &Route::Home);
    // Home is the bottom: Back leaves the app (the platform's).
    assert!(!a.back());
}

// D7, Changes' sheets (C's): a tap outside closes each, and the page's top bar under it is not hit.

#[test]
fn a_tap_outside_the_pull_request_sheet_closes_it_and_reaches_nothing() {
    let mut a = app("pr-sheet");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Changes);
    a.st.changes.pr_sheet = true;
    frames(&mut a, &mut clock, 5);
    // On the sheet's title area: it stays, and nothing under it changes.
    tap(&mut a, &mut clock, 195.0, 844.0 - 330.0 + 40.0);
    assert!(a.st.changes.pr_sheet, "a tap on the sheet keeps it");
    // On the top bar's menu key, above the sheet: it closes, and the drawer does not open.
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(!a.st.changes.pr_sheet);
    assert!(!a.st.drawer.open, "the menu key under the veil took no tap");
    assert_eq!(a.route(), &Route::Changes);
}

#[test]
fn a_tap_outside_the_branch_picker_closes_it_and_reaches_nothing() {
    let mut a = app("branch-picker");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Changes);
    frames(&mut a, &mut clock, 3);
    a.st.changes.picking = true;
    frames(&mut a, &mut clock, 5);
    tap(&mut a, &mut clock, 28.0, 28.0);
    assert!(!a.st.changes.picking);
    assert!(!a.st.drawer.open, "the menu key under the veil took no tap");
    assert_eq!(a.route(), &Route::Changes);
}

// D9: text size and wide screens.

#[test]
fn the_text_size_scales_every_text_and_the_chat_measures_again() {
    let mut a = app("text-size");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    let body = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).size;
    let page_default = a.nav.last().unwrap().1.max;
    a.set_text_size(2);
    frames(&mut a, &mut clock, 5);
    assert_eq!(crate::draw::text_scale(), crate::settings::TEXT_SCALES[2]);
    let large = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).size;
    assert!((large - body * 1.12).abs() < 0.01, "{body} → {large}");
    assert!(a.nav.last().unwrap().1.max > page_default, "the conversation measured again, taller");
    // Back to Default: exactly today's sizes.
    a.set_text_size(1);
    frames(&mut a, &mut clock, 2);
    assert_eq!(crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).size, body);
}

#[test]
fn a_tablet_keeps_a_phone_sized_column_centred() {
    struct Tablet(PathBuf);
    impl Host for Tablet {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
        fn density(&self) -> f32 {
            2.0
        }
    }
    let dir = std::env::temp_dir().join(format!("relay-code-flow-tablet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut a = App::new(Arc::new(Tablet(dir)), 1_000_000_000);
    a.sample();
    a.resize(1600.0, 2560.0, 0.0, 0.0);
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 2);
    // Without the cap: 2560 / 700 = 3.66 px per unit. A phone's look at 2 px per dp is about 2.3.
    assert!(a.canvas.u < 2.4 && a.canvas.u > 2.2, "{}", a.canvas.u);
    // A phone is drawn as before: 1080 wide at 2.625 px per dp fills its width.
    let mut p = app("phone-scale");
    p.resize(1080.0, 2340.0, 0.0, 0.0);
    frames(&mut p, &mut clock, 2);
    assert!((p.canvas.u - 1080.0 / 390.0).abs() < 0.01);
}

// D12: rows that tell you what happened.

#[test]
fn an_agent_that_finished_since_you_looked_gets_a_dot_until_you_open_it() {
    let mut a = app("news");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 3);
    let now = rc_link::link::now_ms();
    // vivid-ferret wrote a minute ago and waits at its prompt; you last looked ten minutes ago.
    let ferret = a.agents.iter().position(|x| x.name == "vivid-ferret").unwrap();
    a.agents[ferret].last_output_at = a.agents.iter().find(|x| x.name == "swift-ocelot").unwrap().last_output_at.clone();
    crate::news::set_seen(&mut a, "vivid-ferret", now - 10 * 60_000);
    let f = a.agents[ferret].clone();
    assert!(crate::news::unseen(&a, &f), "finished since you looked");
    // One still at work has no dot, however long since you looked.
    crate::news::set_seen(&mut a, "bold-puffin", now - 60 * 60_000);
    let puffin = a.agent("bold-puffin").cloned().unwrap();
    assert!(!crate::news::unseen(&a, &puffin), "at work: nothing to see yet");
    // Open its chat, come back: the dot is gone.
    a.open("vivid-ferret");
    frames(&mut a, &mut clock, 3);
    assert!(a.back());
    frames(&mut a, &mut clock, 3);
    assert!(!crate::news::unseen(&a, &f));
    // Its row shows its latest words, from the conversation in memory.
    assert_eq!(crate::news::preview(&mut a, "vivid-ferret").as_deref(), Some("All five review findings are fixed, with a test each."));
}

#[test]
fn a_first_seen_agent_starts_seen() {
    let mut a = app("news-first");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 3);
    // Every sample agent is new to this phone but two the sample marked: no old news lights up.
    let lit: Vec<String> = a.agents.clone().iter().filter(|x| crate::news::unseen(&a, x)).map(|x| x.name.clone()).collect();
    assert_eq!(lit, ["umber-zebra", "swift-ocelot"]);
}

// D11: Back closes every overlay, and leaving a page leaves nothing open for the next visit.

#[test]
fn back_closes_changes_sheets_one_at_a_time_then_leaves_the_page() {
    let mut a = app("back-changes");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Changes);
    frames(&mut a, &mut clock, 3);
    a.st.changes.picking = true;
    a.st.changes.pr_sheet = true;
    assert!(a.back());
    assert!(!a.st.changes.pr_sheet && a.st.changes.picking, "the top one first");
    assert!(a.back());
    assert!(!a.st.changes.picking);
    assert_eq!(a.route(), &Route::Changes);
    assert!(a.back());
    assert_eq!(a.route(), &Route::Home);
}

#[test]
fn leaving_a_page_leaves_none_of_its_overlays_for_the_next_visit() {
    let mut a = app("leave-clean");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Changes);
    frames(&mut a, &mut clock, 3);
    a.st.changes.pr_sheet = true;
    // Away by the drawer (no Back): the next visit opens on the page, not the sheet.
    a.reset_to(Route::Home);
    a.push(Route::Changes);
    frames(&mut a, &mut clock, 3);
    assert!(!a.st.changes.pr_sheet);
    a.reset_to(Route::Home);
    a.push(Route::Inbox);
    a.st.inbox.denying = Some(7);
    a.reset_to(Route::Home);
    a.push(Route::Inbox);
    assert_eq!(a.st.inbox.denying, None);
}

#[test]
fn back_closes_the_deny_sheet_and_the_forget_confirm_on_their_pages() {
    let mut a = app("back-deny");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Inbox);
    frames(&mut a, &mut clock, 3);
    a.st.inbox.denying = Some(7);
    assert!(a.back());
    assert_eq!((a.st.inbox.denying, a.route()), (None, &Route::Inbox));
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    frames(&mut a, &mut clock, 3);
    tap(&mut a, &mut clock, 200.0, FORGET_TOP + 23.0);
    assert!(crate::settings::confirming(&a));
    assert!(a.back());
    assert!(!crate::settings::confirming(&a));
    assert_eq!(a.route(), &Route::Settings);
    // A flag left on another page never takes a Back here.
    a.st.inbox.denying = Some(7);
    assert!(a.back());
    assert_eq!(a.route(), &Route::Home);
}

// D13: when the card can't help.

#[test]
fn the_key_toggle_opens_the_strip_above_the_box_and_closes_it() {
    let mut a = app("keys");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    frames(&mut a, &mut clock, 5);
    let box_top = a.st.chat.box_top;
    let lh = crate::draw::Text::new(crate::theme::ty::BODY, crate::theme::INK).line(1.4).lh();
    let box_h = 14.0 + lh + 10.0 + 34.0 + 10.0;
    let (kx, ky) = (12.0 + (390.0 - 24.0) - 10.0 - 34.0 - 8.0 - 34.0, box_top + box_h - 44.0);
    tap(&mut a, &mut clock, kx + 17.0, ky + 17.0);
    assert!(a.st.keys.open, "the toggle opened the strip");
    // The box stays at the foot; the strip takes its own room above it, the conversation ending higher.
    assert!((box_top - a.st.chat.box_top).abs() < 0.5, "the box stayed where it was");
    let max_closed = a.nav.last().unwrap().1.max;
    a.st.keys.open = false;
    frames(&mut a, &mut clock, 3);
    let max_without = a.nav.last().unwrap().1.max;
    assert!((max_closed - max_without - crate::keys::STRIP_H).abs() < 0.5, "the page made room for the strip, no more");
    a.st.keys.open = true;
    frames(&mut a, &mut clock, 3);
    assert!(a.focus.id.is_none() && !a.st.chat.model_sheet, "nothing else took the tap");
    let ky = a.st.chat.box_top + box_h - 44.0;
    tap(&mut a, &mut clock, kx + 17.0, ky + 17.0);
    assert!(!a.st.keys.open);
}

#[test]
fn the_chat_menu_opens_the_terminal_and_back_comes_back() {
    let mut a = app("terminal");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.agents.menu = Some("bold-puffin".into());
    frames(&mut a, &mut clock, 5);
    // The menu's second row: Open terminal (after Find in chat).
    tap(&mut a, &mut clock, 280.0, 52.0 + 4.0 + 48.0 + 24.0);
    assert_eq!(a.route(), &Route::Terminal("bold-puffin".into()));
    crate::terminal::sample(&mut a, "bold-puffin");
    frames(&mut a, &mut clock, 3);
    // A key at the foot reads the screen again a moment later.
    let before = crate::terminal::next_read(&a);
    tap(&mut a, &mut clock, 40.0, 844.0 - (crate::keys::STRIP_H + 8.0) + 6.0 + 20.0);
    assert!(crate::terminal::next_read(&a) <= before.min(a.now_ns + 250_000_000));
    assert_eq!(a.route(), &Route::Terminal("bold-puffin".into()), "the key went to the terminal, not under it");
    assert!(a.back());
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
}

// D15: a terminal you can read.

fn terminal_open(name: &str) -> (App, u64) {
    let mut a = app(name);
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.push(Route::Terminal("bold-puffin".into()));
    crate::terminal::sample(&mut a, "bold-puffin");
    frames(&mut a, &mut clock, 5);
    (a, clock)
}

#[test]
fn the_terminal_text_is_readable_and_a_minus_a_plus_size_it() {
    let (mut a, mut clock) = terminal_open("term-size");
    let size = |a: &App| if a.st.terminal.size > 0.0 { a.st.terminal.size } else { 11.0 };
    assert!(size(&a) >= 11.0, "11 at least, unless made smaller");
    // A+ is the bar's last key, A− the one before it.
    tap(&mut a, &mut clock, 390.0 - 6.0 - 22.0, 6.0 + 22.0);
    assert_eq!(size(&a), 12.0);
    for _ in 0..10 {
        tap(&mut a, &mut clock, 390.0 - 6.0 - 44.0 - 22.0, 6.0 + 22.0);
    }
    assert_eq!(size(&a), 9.0, "no smaller than 9");
    assert_eq!(a.route(), &Route::Terminal("bold-puffin".into()));
}

#[test]
fn a_wide_screen_scrolls_sideways_and_the_drawer_stays_shut() {
    let (mut a, mut clock) = terminal_open("term-side");
    let (off, max) = crate::terminal::sideways(&a);
    assert!(max > 0.0 && off == 0.0, "100 columns are wider than the page: {max}");
    // A sideways drag on the screen, from near the left edge.
    let (y, from) = (700.0, 20.0);
    clock += 16_666_666;
    a.touch(Touch { id: 1, phase: Phase::Down, x: from + 200.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 1);
    for i in 1..=10 {
        clock += 16_666_666;
        a.touch(Touch { id: 1, phase: Phase::Move, x: from + 200.0 - 15.0 * i as f32, y, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    clock += 100_000_000;
    a.touch(Touch { id: 1, phase: Phase::Up, x: from + 50.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 20);
    assert!(crate::terminal::sideways(&a).0 > 50.0, "it scrolled: {:?}", crate::terminal::sideways(&a));
    assert!(!a.st.drawer.open);
    // From the very edge, rightwards: the screen's, not the drawer's.
    clock += 16_666_666;
    a.touch(Touch { id: 2, phase: Phase::Down, x: 18.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 1);
    for i in 1..=10 {
        clock += 16_666_666;
        a.touch(Touch { id: 2, phase: Phase::Move, x: 18.0 + 15.0 * i as f32, y, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    clock += 100_000_000;
    a.touch(Touch { id: 2, phase: Phase::Up, x: 168.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 20);
    assert!(!a.st.drawer.open, "an edge swipe on the screen scrolls it back");
    assert!(crate::terminal::sideways(&a).0 < 50.0);
    assert_eq!(a.route(), &Route::Terminal("bold-puffin".into()));
}

// D14: every state says what it is.

#[test]
fn away_from_the_pc_the_pages_say_so_and_no_row_claims_work() {
    let mut a = app("reach");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 2);
    assert!(crate::home::reach(&a).is_none(), "nothing to say while it answers");
    a.screens.insert("swift-ocelot".into(), crate::screen::read("· Crunching… (1m 6s · ↓ 6.3k tokens · thought for 2s)\n────\n❯ \n────\n  ⏵⏵ auto mode on (shift+tab to cycle) · esc to interrupt"));
    // Its chat open, its terminal's foot is trusted: it works.
    a.open("swift-ocelot");
    let ocelot = a.agent("swift-ocelot").cloned().unwrap();
    assert_eq!(a.agent_words(&ocelot), "working");
    a.set_link_state(rc_link::LinkState::Offline { reason: "No route".into(), retry_at_ms: 0 });
    frames(&mut a, &mut clock, 2);
    assert_eq!(crate::home::reach(&a).map(|r| r.1).as_deref(), Some("Out of reach · trying now"));
    assert_eq!(a.agent_words(&ocelot), "ready", "its terminal's last word is old news");
    a.set_link_state(rc_link::LinkState::Connecting { attempt: 1 });
    a.agents.clear();
    a.projects.clear();
    assert_eq!(crate::home::reach(&a).map(|r| r.1).as_deref(), Some("Connecting…"));
}

#[test]
fn find_in_chat_from_the_menu_hands_the_chat_its_find() {
    let mut a = app("find");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.st.agents.menu = Some("bold-puffin".into());
    frames(&mut a, &mut clock, 5);
    tap(&mut a, &mut clock, 280.0, 52.0 + 4.0 + 24.0);
    assert!(a.st.agents.menu.is_none());
    frames(&mut a, &mut clock, 2);
    assert!(a.focus.id.is_some(), "the chat's find took the keyboard");
    assert_eq!(a.route(), &Route::Chat("bold-puffin".into()));
}

// D17: ready for the phone.

#[test]
fn check_for_updates_now_says_what_it_can() {
    let mut a = app("check-now");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    frames(&mut a, &mut clock, 3);
    // Tests, like shots, never reach the PC: the row says so in words rather than doing nothing.
    tap(&mut a, &mut clock, 200.0, CHECK_TOP + 26.0);
    assert_eq!(a.toast_words(), Some("Your PC is out of reach. Try again once it answers"));
    assert_eq!(a.route(), &Route::Settings);
    assert!(!crate::settings::confirming(&a), "the Forget key under it was not hit");
}

// D11 follow-up: C18's Send back sheet.

#[test]
fn back_closes_the_send_back_sheet_on_the_task_and_in_the_inbox() {
    let mut a = app("send-back");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Board);
    a.push(Route::Task(41));
    frames(&mut a, &mut clock, 3);
    crate::board::send_back(&mut a, 41, "Relay Code: Designs open on artboards");
    frames(&mut a, &mut clock, 2);
    assert!(a.back());
    assert!(a.st.board.back.is_none(), "Back closed the sheet");
    assert_eq!(a.route(), &Route::Task(41), "and stayed on the task");
    // In the Inbox too.
    a.reset_to(Route::Home);
    a.push(Route::Inbox);
    frames(&mut a, &mut clock, 2);
    crate::board::send_back(&mut a, 41, "Relay Code: Designs open on artboards");
    assert!(a.back());
    assert!(a.st.board.back.is_none());
    assert_eq!(a.route(), &Route::Inbox);
    // Elsewhere a stray sheet never takes a Back.
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    a.st.board.back = Some((41, "x".into()));
    assert!(a.back());
    assert_eq!(a.route(), &Route::Home);
}

#[test]
fn leaving_the_task_leaves_no_send_back_sheet_for_the_next_visit() {
    let mut a = app("send-back-leave");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Task(41));
    frames(&mut a, &mut clock, 2);
    crate::board::send_back(&mut a, 41, "Relay Code: Designs open on artboards");
    // Away by the drawer, then back to the task.
    a.reset_to(Route::Home);
    a.push(Route::Task(41));
    frames(&mut a, &mut clock, 2);
    assert!(a.st.board.back.is_none());
}

// D20: Default follows the phone's text size.

#[test]
fn default_follows_the_phones_font_scale_within_bounds_and_small_large_override() {
    use crate::settings::text_scale;
    assert_eq!(text_scale(1, 1.0), 1.0);
    assert_eq!(text_scale(1, 1.15), 1.15);
    assert_eq!(text_scale(1, 2.0), 1.3, "a huge phone setting is held to what the layout takes");
    assert_eq!(text_scale(1, 0.5), 0.85);
    assert_eq!(text_scale(1, f32::NAN), 1.0, "an unreadable setting is the default");
    assert_eq!(text_scale(0, 1.3), 0.92, "Small is Small whatever the phone says");
    assert_eq!(text_scale(2, 0.85), 1.12);
    // In the app: a phone at 1.3 sets the type at 1.3 and its rows grow to hold three lines.
    let mut a = app("font-scale");
    let mut clock = 1_000_000_000;
    a.font_scale = 1.3;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 2);
    assert!((crate::draw::text_scale() - 1.3).abs() < 0.001);
    assert!((crate::drawer::agent_row_h() - 72.0 * 1.3).abs() < 0.01);
}

#[test]
fn an_edge_swipe_under_a_sheet_does_not_open_the_drawer() {
    let mut a = app("edge-under-sheet");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Board);
    frames(&mut a, &mut clock, 3);
    crate::board::new_task(&mut a);
    frames(&mut a, &mut clock, 3);
    let y = 300.0;
    clock += 16_666_666;
    a.touch(Touch { id: 1, phase: Phase::Down, x: 6.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 1);
    for i in 1..=10 {
        clock += 16_666_666;
        a.touch(Touch { id: 1, phase: Phase::Move, x: 6.0 + 20.0 * i as f32, y, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    clock += 100_000_000;
    a.touch(Touch { id: 1, phase: Phase::Up, x: 206.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 20);
    assert!(!a.st.drawer.open, "the sheet's, not the drawer's");
    // With no sheet up, the same swipe opens it.
    a.st.board.new_task = None;
    frames(&mut a, &mut clock, 3);
    clock += 16_666_666;
    a.touch(Touch { id: 2, phase: Phase::Down, x: 6.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 1);
    for i in 1..=10 {
        clock += 16_666_666;
        a.touch(Touch { id: 2, phase: Phase::Move, x: 6.0 + 20.0 * i as f32, y, t_ns: clock });
        frames(&mut a, &mut clock, 1);
    }
    clock += 100_000_000;
    a.touch(Touch { id: 2, phase: Phase::Up, x: 206.0, y, t_ns: clock });
    frames(&mut a, &mut clock, 20);
    assert!(a.st.drawer.open);
}

/// How long a Terminal frame takes with a full 200-row, 120-column screen of box drawing and
/// marks: `cargo test --release terminal_frame_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn terminal_frame_cost() {
    let mut a = app("bench-terminal");
    a.open("bold-puffin");
    a.push(Route::Terminal("bold-puffin".into()));
    crate::terminal::sample(&mut a, "bold-puffin");
    let row = format!("│ ❯ {} ✔ │", "Opus 5.5 for complex tasks · Sonnet 5.5 for daily coding ".repeat(2));
    let screen: Vec<String> = (0..200).map(|i| if i % 10 == 0 { format!("╭{}╮", "─".repeat(118)) } else { row.clone() }).collect();
    crate::terminal::set_text(&mut a, &screen.join("\n"));
    let mut clock = 1_000_000_000;
    frames(&mut a, &mut clock, 5);
    let n = 200;
    let t = std::time::Instant::now();
    frames(&mut a, &mut clock, n);
    println!("terminal frame: {:.0} µs", t.elapsed().as_secs_f64() * 1e6 / n as f64);
}

// D22: spry-falcon's review of D's area.

#[test]
fn later_while_downloading_stops_it_and_nothing_installs() {
    let mut a = app("later-stops");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    crate::update::sample(&mut a, crate::update::BUILD + 1, Some(7_300_000));
    frames(&mut a, &mut clock, 5);
    let y = update_sheet_keys_y(&mut a);
    tap(&mut a, &mut clock, 80.0, y);
    assert!(!a.st.update.open && !a.st.update.downloading);
    assert!(crate::update::stopped(&a), "the download thread is told to stop");
    // Whatever it ends with is never installed.
    crate::update::finish_download(&mut a, Ok(std::path::PathBuf::from("/tmp/x.apk")));
    frames(&mut a, &mut clock, 3);
    assert!(a.st.update.ready.is_none());
    // Back while downloading does the same.
    crate::update::sample(&mut a, crate::update::BUILD + 1, Some(1_000));
    assert!(a.back());
    assert!(!a.st.update.open && !a.st.update.downloading && crate::update::stopped(&a));
}

#[test]
fn forget_this_pc_leaves_nothing_of_it_but_the_phones_own_choices() {
    let mut a = app("forget-all");
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    frames(&mut a, &mut clock, 2);
    a.set_text_size(2);
    a.set_notifies(crate::settings::Notify::Done, false);
    a.st.terminal.size = 13.0;
    a.holds = 3;
    crate::update::sample(&mut a, crate::update::BUILD + 1, None);
    assert!(!a.st.board.tasks.is_empty() && !a.earlier.is_empty() && a.repo.is_some());
    a.unpair();
    assert_eq!(a.route(), &Route::Pair);
    assert!(a.repo.is_none() && a.agents.is_empty() && a.earlier.is_empty() && a.projects.is_empty());
    assert!(a.st.board.tasks.is_empty(), "the board's tasks were the old PC's");
    assert!(a.st.update.latest.is_none() && !a.st.update.open, "its update offer too");
    assert_eq!(a.holds, 0);
    assert!(a.screens.is_empty() && a.convos.is_empty());
    // The phone's own choices stay.
    assert_eq!(a.text_size(), 2);
    assert!(!a.notifies(crate::settings::Notify::Done));
    assert_eq!(a.st.terminal.size, 13.0);
}

#[test]
fn switching_repo_leaves_another_repos_terminal() {
    let mut a = app("repo-terminal");
    let mut clock = 1_000_000_000;
    a.open("bold-puffin");
    a.push(Route::Terminal("bold-puffin".into()));
    frames(&mut a, &mut clock, 2);
    a.set_repo(2);
    assert_eq!(a.route(), &Route::Home, "its keys would type into the old repo's agent");
    // The same repo keeps you there.
    a.set_repo(1);
    a.open("bold-puffin");
    a.push(Route::Terminal("bold-puffin".into()));
    a.set_repo(1);
    assert_eq!(a.route(), &Route::Terminal("bold-puffin".into()));
}

#[test]
fn with_notifications_off_in_android_the_switches_read_off_and_lead_there() {
    struct Blocked(PathBuf, std::sync::atomic::AtomicUsize);
    impl Host for Blocked {
        fn data_dir(&self) -> PathBuf {
            self.0.clone()
        }
        fn log(&self, _text: &str) {}
        fn can_notify(&self) -> Option<bool> {
            Some(false)
        }
        fn open_notification_settings(&self) {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dir = std::env::temp_dir().join(format!("relay-code-flow-blocked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let host = Arc::new(Blocked(dir, std::sync::atomic::AtomicUsize::new(0)));
    let mut a = App::new(host.clone(), 1_000_000_000);
    a.resize(390.0, 844.0, 0.0, 0.0);
    a.sample();
    let mut clock = 1_000_000_000;
    a.reset_to(Route::Home);
    a.push(Route::Settings);
    frames(&mut a, &mut clock, 3);
    // The first switch row sits under the row that says they are off.
    tap(&mut a, &mut clock, 200.0, NOTIFY_TOP + 64.0 + 26.0);
    assert_eq!(host.1.load(std::sync::atomic::Ordering::SeqCst), 1, "a switch leads to Android's settings");
    assert!(a.notifies(crate::settings::Notify::Asks), "and changes nothing here");
}
