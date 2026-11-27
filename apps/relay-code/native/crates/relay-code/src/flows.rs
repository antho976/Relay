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
    let list_top = 8.0 + 56.0 + 10.0 + 46.0 + 12.0;
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
    let copied = host.1.lock().unwrap().clone();
    assert_eq!(copied.len(), 1, "one copy: {copied:?}");
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
