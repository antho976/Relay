//! Byte-exact fixtures for the scrcpy v4.1 wire protocol (D59). Layouts mirror the
//! pinned upstream sources; if any of these fail after a server-jar bump, the
//! protocol moved and mirror.rs must be re-transcribed against the new version.

use relay_core::mirror::{
    self, parse_codec_id, parse_stream_unit, InputMsg, StreamUnit, ACTION_DOWN, ACTION_MOVE, ACTION_UP, CODEC_ID_H264, INJECT_TEXT_MAX_LENGTH, KEYCODE_APP_SWITCH,
    KEYCODE_HOME, KEYCODE_POWER, KEYCODE_VOLUME_DOWN, KEYCODE_VOLUME_UP, KEYCODE_WAKEUP,
    META_CTRL_ON, POINTER_ID_GENERIC_FINGER,
};

// -- launch args ------------------------------------------------------------

#[test]
fn server_args_pin_version_and_required_options() {
    let args = mirror::server_shell_args("SERIAL1", 0xabcd_1234, &mirror::ServerLaunch::new(mirror::CAPTURE_MIN, 8_000_000));
    assert_eq!(args[0], "-s");
    assert_eq!(args[1], "SERIAL1");
    assert!(args.contains(&"4.1".to_string()));
    assert!(args.contains(&"scid=abcd1234".to_string()));
    assert!(args.contains(&"tunnel_forward=true".to_string()));
    assert!(args.contains(&"audio=false".to_string()));
    assert!(args.contains(&"video_codec=h264".to_string()));
    assert!(args.iter().any(|a| a.starts_with("CLASSPATH=/data/local/tmp/")));
    // The version string sits right after the server class name — the server
    // parses positionally.
    let class = args.iter().position(|a| a == "com.genymobile.scrcpy.Server").unwrap();
    assert_eq!(args[class + 1], "4.1");
}

#[test]
fn capture_size_snaps_up_the_ladder_and_saturates() {
    // Exact rungs pass through; anything between takes the next one UP, so a panel that
    // outgrew a rung always gets more pixels than it renders, never fewer.
    assert_eq!(mirror::capture_size_for(1024), 1024);
    assert_eq!(mirror::capture_size_for(1025), 1280);
    assert_eq!(mirror::capture_size_for(1440), 1600);
    assert_eq!(mirror::capture_size_for(1920), 1920);
    // Past the top rung we ask for the top rung — never an unbounded value a device
    // encoder might refuse.
    assert_eq!(mirror::capture_size_for(4000), 2560);
    assert_eq!(mirror::capture_size_for(0), 1024);
    // Sorted, and the panel's "did the rung grow?" check depends on it staying so.
    assert!(mirror::CAPTURE_SIZES.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn bit_rate_scales_with_capture_size_and_is_capped() {
    assert_eq!(mirror::capture_bit_rate(1024), 8_000_000);
    assert_eq!(mirror::capture_bit_rate(1920), 15_000_000);
    assert_eq!(mirror::capture_bit_rate(2560), 20_000_000);
    // Off-ladder input snaps first, so the rate always matches the rung actually used.
    assert_eq!(mirror::capture_bit_rate(1100), mirror::capture_bit_rate(1280));
}

#[test]
fn server_args_carry_the_requested_capture_size() {
    let args = mirror::server_shell_args("S", 1, &mirror::ServerLaunch::new(1600, mirror::capture_bit_rate(1600)));
    assert!(args.contains(&"max_size=1600".to_string()));
    assert!(args.contains(&"video_bit_rate=12500000".to_string()));
    // An off-ladder request can never reach the server: the builder snaps it.
    let odd = mirror::server_shell_args("S", 1, &mirror::ServerLaunch::new(1300, 8_000_000));
    assert!(odd.contains(&"max_size=1600".to_string()));
}

#[test]
fn forward_args_use_ephemeral_port_and_scid_socket() {
    let args = mirror::forward_args("S", 0x00ff_0001);
    assert_eq!(args, vec!["-s", "S", "forward", "tcp:0", "localabstract:scrcpy_00ff0001"]);
    // `--remove` names the local end adb printed; a remote socket name is refused.
    let rm = mirror::forward_remove_args("S", 40123);
    assert_eq!(rm, vec!["-s", "S", "forward", "--remove", "tcp:40123"]);
}

#[test]
fn push_args_target_relay_owned_path() {
    let args = mirror::push_args("S", "/res/scrcpy-server-v4.1");
    // Positional, so the assertion moved when `--sync` was added — which is the point of
    // asserting the whole argv instead: the jar is 733 KB over USB on every mirror start
    // unless adb is told to compare first, and losing that flag is silent.
    assert_eq!(
        args,
        vec![
            "-s",
            "S",
            "push",
            "--sync",
            "/res/scrcpy-server-v4.1",
            "/data/local/tmp/scrcpy-server-relay.jar",
        ]
    );
}

// -- stream parsing ---------------------------------------------------------

#[test]
fn codec_id_h264() {
    assert_eq!(parse_codec_id(b"h264"), CODEC_ID_H264);
}

#[test]
fn session_packet_carries_dimensions() {
    let mut h = [0u8; 12];
    h[0] = 0x80;
    h[4..8].copy_from_slice(&1080u32.to_be_bytes());
    h[8..12].copy_from_slice(&2400u32.to_be_bytes());
    assert_eq!(parse_stream_unit(&h), StreamUnit::Session { width: 1080, height: 2400 });
}

#[test]
fn media_header_flags_and_pts() {
    // config packet: bit 62, pts meaningless, len 32
    let mut h = [0u8; 12];
    h[..8].copy_from_slice(&(1u64 << 62).to_be_bytes());
    h[8..12].copy_from_slice(&32u32.to_be_bytes());
    assert_eq!(
        parse_stream_unit(&h),
        StreamUnit::Media { config: true, key: false, pts: 0, len: 32 }
    );

    // key frame with a pts: bit 61 | 123456, len 70000
    let mut h = [0u8; 12];
    h[..8].copy_from_slice(&((1u64 << 61) | 123_456).to_be_bytes());
    h[8..12].copy_from_slice(&70_000u32.to_be_bytes());
    assert_eq!(
        parse_stream_unit(&h),
        StreamUnit::Media { config: false, key: true, pts: 123_456, len: 70_000 }
    );

    // plain delta frame
    let mut h = [0u8; 12];
    h[..8].copy_from_slice(&99u64.to_be_bytes());
    h[8..12].copy_from_slice(&1500u32.to_be_bytes());
    assert_eq!(
        parse_stream_unit(&h),
        StreamUnit::Media { config: false, key: false, pts: 99, len: 1500 }
    );
}

// -- control encoders -------------------------------------------------------

#[test]
fn touch_down_is_byte_exact() {
    let m = mirror::touch(ACTION_DOWN, 100, 200, 1080, 2400, 1.0);
    #[rustfmt::skip]
    let expect: [u8; 32] = [
        2,                                  // TYPE_INJECT_TOUCH_EVENT
        0,                                  // ACTION_DOWN
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // pointer id -1 (mouse)
        0x00, 0x00, 0x00, 0x64,             // x = 100
        0x00, 0x00, 0x00, 0xc8,             // y = 200
        0x04, 0x38,                         // w = 1080
        0x09, 0x60,                         // h = 2400
        0xff, 0xff,                         // pressure 1.0 -> u16fp max
        0x00, 0x00, 0x00, 0x01,             // action_button = PRIMARY
        0x00, 0x00, 0x00, 0x01,             // buttons = PRIMARY
    ];
    assert_eq!(m, expect);
}

#[test]
fn touch_up_releases_buttons_and_pressure() {
    let m = mirror::touch(ACTION_UP, 5, 6, 1080, 2400, 0.0);
    assert_eq!(m[1], 1);
    assert_eq!(&m[22..24], &[0x00, 0x00]); // pressure 0
    assert_eq!(&m[24..28], &[0, 0, 0, 1]); // action_button stays PRIMARY
    assert_eq!(&m[28..32], &[0, 0, 0, 0]); // buttons cleared on UP
}

#[test]
fn touch_move_keeps_buttons_held_and_changes_no_button() {
    let m = mirror::touch(ACTION_MOVE, 5, 6, 1080, 2400, 1.0);
    assert_eq!(m[1], 2);
    assert_eq!(&m[28..32], &[0, 0, 0, 1]); // still held
    // action_button is the button whose state CHANGED — a drag changes none. Claiming
    // PRIMARY here fabricates a press edge the server turns into ACTION_BUTTON_PRESS.
    assert_eq!(&m[24..28], &[0, 0, 0, 0]);
}

#[test]
fn touch_pointer_allows_a_finger_and_explicit_button_state() {
    let m = mirror::touch_pointer(POINTER_ID_GENERIC_FINGER, ACTION_DOWN, 1, 2, 10, 20, 0.5, 0, 0);
    assert_eq!(&m[2..10], &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe]); // -2
    assert_eq!(&m[22..24], &[0x80, 0x00]); // 0.5 -> u16fp
    assert_eq!(&m[24..28], &[0, 0, 0, 0]);
    assert_eq!(&m[28..32], &[0, 0, 0, 0]);
    // The mouse convenience is exactly this with mouse conventions filled in.
    assert_eq!(
        mirror::touch(ACTION_DOWN, 1, 2, 10, 20, 1.0),
        mirror::touch_pointer(u64::MAX, ACTION_DOWN, 1, 2, 10, 20, 1.0, 1, 1)
    );
}

#[test]
fn scroll_is_byte_exact() {
    let m = mirror::scroll(540, 1200, 1080, 2400, 0.0, -1.0);
    #[rustfmt::skip]
    let expect: [u8; 21] = [
        3,                                  // TYPE_INJECT_SCROLL_EVENT
        0x00, 0x00, 0x02, 0x1c,             // x = 540
        0x00, 0x00, 0x04, 0xb0,             // y = 1200
        0x04, 0x38,                         // w = 1080
        0x09, 0x60,                         // h = 2400
        0x00, 0x00,                         // hscroll 0
        0x80, 0x00,                         // vscroll -1.0 -> i16fp min
        0x00, 0x00, 0x00, 0x00,             // buttons none
    ];
    assert_eq!(m, expect);
    // +1.0 saturates to 0x7fff, not 0x8000
    let m = mirror::scroll(0, 0, 1, 1, 0.0, 1.0);
    assert_eq!(&m[15..17], &[0x7f, 0xff]);
}

#[test]
fn keycode_is_byte_exact() {
    // ENTER (66) down
    let m = mirror::keycode(ACTION_DOWN, 66, 0, 0);
    #[rustfmt::skip]
    let expect: [u8; 14] = [
        0, 0,
        0x00, 0x00, 0x00, 0x42,             // keycode 66
        0x00, 0x00, 0x00, 0x00,             // repeat
        0x00, 0x00, 0x00, 0x00,             // meta
    ];
    assert_eq!(m, expect);
}

#[test]
fn text_prefixes_length_and_truncates_on_char_boundary() {
    assert_eq!(mirror::text("hi"), vec![1, 0, 0, 0, 2, b'h', b'i']);
    // 300-byte cap must not split a multi-byte char. A leading ASCII byte puts every
    // é (2 bytes) on odd offsets, so the one at 299..301 straddles the cap and has to
    // be dropped whole: 1 + 149 * 2 = 299 bytes. Without the offset 300 is already a
    // boundary and a naive byte slice would pass.
    let s = format!("a{}", "é".repeat(150));
    let m = mirror::text(&s);
    let len = u32::from_be_bytes(m[1..5].try_into().unwrap()) as usize;
    assert_eq!(len, 299);
    assert_eq!(m.len(), 5 + len);
    assert_eq!(&m[5..], &s.as_bytes()[..299]);
}

#[test]
fn text_chunks_never_truncate_and_stay_under_the_cap() {
    // 700 ASCII bytes = three messages (300 + 300 + 100), all of it sent.
    let s = "a".repeat(700);
    let chunks = mirror::text_chunks(&s);
    assert_eq!(chunks.len(), 3);
    let mut payload = Vec::new();
    for c in &chunks {
        assert_eq!(c[0], 1);
        let len = u32::from_be_bytes(c[1..5].try_into().unwrap()) as usize;
        assert!(len <= INJECT_TEXT_MAX_LENGTH);
        assert_eq!(c.len(), 5 + len);
        payload.extend_from_slice(&c[5..]);
    }
    assert_eq!(payload, s.as_bytes());
    assert!(mirror::text_chunks("").is_empty());

    // Multi-byte chars never split: "a" + 150 * "é" = 301 bytes, and the é at 299..301
    // straddles the cap, so the split is 299 + 2 rather than 300 + 1.
    let s = format!("a{}", "é".repeat(150));
    let chunks = mirror::text_chunks(&s);
    let lens: Vec<usize> = chunks.iter().map(|c| c.len() - 5).collect();
    assert_eq!(lens, [299, 2]);
    for c in &chunks {
        assert!(std::str::from_utf8(&c[5..]).is_ok());
    }
    assert_eq!(
        chunks.iter().flat_map(|c| c[5..].to_vec()).collect::<Vec<_>>(),
        s.as_bytes()
    );
}

#[test]
fn encode_input_text_sends_the_whole_string() {
    // The single-message encoder truncates by design; the input seam must not.
    let s = "z".repeat(700);
    assert_eq!(mirror::text(&s).len(), 5 + 300);
    let bytes = mirror::encode_input(&InputMsg::Text { text: s.clone() });
    assert_eq!(bytes.len(), 3 * 5 + 700);
    assert_eq!(bytes, mirror::text_chunks(&s).concat());
}

// -- navigation and system keys ---------------------------------------------

#[test]
fn key_press_is_down_then_up_byte_exact() {
    let m = mirror::key_press(KEYCODE_HOME, 0);
    #[rustfmt::skip]
    let expect: [u8; 28] = [
        0, 0, 0x00, 0x00, 0x00, 0x03, 0, 0, 0, 0, 0, 0, 0, 0, // HOME down
        0, 1, 0x00, 0x00, 0x00, 0x03, 0, 0, 0, 0, 0, 0, 0, 0, // HOME up
    ];
    assert_eq!(m, expect);
    // meta rides on both halves
    let m = mirror::key_press(29, META_CTRL_ON); // Ctrl+A
    assert_eq!(&m[10..14], &[0x00, 0x00, 0x10, 0x00]);
    assert_eq!(&m[24..28], &[0x00, 0x00, 0x10, 0x00]);
}

#[test]
fn back_press_is_down_then_up() {
    // BACK_OR_SCREEN_ON, not keycode 4: the server presses Back when the screen is on
    // and wakes it when it is off.
    assert_eq!(mirror::encode_input(&InputMsg::Back), vec![4, 0, 4, 1]);
}

#[test]
fn navigation_variants_encode_the_right_keycodes() {
    let cases: &[(InputMsg, u32)] = &[
        (InputMsg::Home, KEYCODE_HOME),
        (InputMsg::AppSwitch, KEYCODE_APP_SWITCH),
        (InputMsg::Power, KEYCODE_POWER),
        (InputMsg::ScreenOn, KEYCODE_WAKEUP),
        (InputMsg::VolumeUp, KEYCODE_VOLUME_UP),
        (InputMsg::VolumeDown, KEYCODE_VOLUME_DOWN),
    ];
    for (msg, kc) in cases {
        let bytes = mirror::encode_input(msg);
        assert_eq!(bytes.len(), 28, "{msg:?}");
        assert_eq!(bytes, mirror::key_press(*kc, 0).to_vec(), "{msg:?}");
    }
    // The values themselves, pinned: these are Android platform constants and a wrong
    // one is a different button, not an error.
    assert_eq!((KEYCODE_HOME, KEYCODE_APP_SWITCH, KEYCODE_POWER), (3, 187, 26));
    assert_eq!((KEYCODE_VOLUME_UP, KEYCODE_VOLUME_DOWN, KEYCODE_WAKEUP), (24, 25, 224));
    assert_eq!(mirror::encode_input(&InputMsg::KeyPress { keycode: 66, meta: 0 }).len(), 28);
}

#[test]
fn panel_rotate_power_and_reset_are_byte_exact() {
    assert_eq!(mirror::expand_notification_panel(), [5]);
    assert_eq!(mirror::expand_settings_panel(), [6]);
    assert_eq!(mirror::collapse_panels(), [7]);
    assert_eq!(mirror::set_display_power(true), [10, 1]);
    assert_eq!(mirror::set_display_power(false), [10, 0]);
    assert_eq!(mirror::rotate_device(), [11]);
    assert_eq!(mirror::reset_video(), [17]);

    assert_eq!(mirror::encode_input(&InputMsg::Notifications), vec![5]);
    assert_eq!(mirror::encode_input(&InputMsg::QuickSettings), vec![6]);
    assert_eq!(mirror::encode_input(&InputMsg::Collapse), vec![7]);
    assert_eq!(mirror::encode_input(&InputMsg::DisplayPower { on: false }), vec![10, 0]);
    assert_eq!(mirror::encode_input(&InputMsg::Rotate), vec![11]);
    assert_eq!(mirror::encode_input(&InputMsg::ResetVideo), vec![17]);
}

// -- clipboard --------------------------------------------------------------

#[test]
fn set_clipboard_is_byte_exact() {
    let m = mirror::set_clipboard(true, "hi");
    #[rustfmt::skip]
    let expect: Vec<u8> = vec![
        9,                                              // TYPE_SET_CLIPBOARD
        0, 0, 0, 0, 0, 0, 0, 0,                         // sequence: 0, no ack wanted
        1,                                              // paste
        0x00, 0x00, 0x00, 0x02,                         // length
        b'h', b'i',
    ];
    assert_eq!(m, expect);
    // paste off
    assert_eq!(mirror::set_clipboard(false, ""), vec![9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    let msg = InputMsg::SetClipboard { text: "hi".into(), paste: true };
    assert_eq!(mirror::encode_input(&msg), mirror::set_clipboard(true, "hi"));
}

#[test]
fn set_clipboard_caps_on_a_char_boundary() {
    // 256 KiB minus the 14-byte header. A multi-byte char straddling the cap must be
    // dropped whole, or the device decodes garbage at the tail.
    let max = (1 << 18) - 14;
    assert_eq!(mirror::SET_CLIPBOARD_MAX_LENGTH, max);
    // "€" is 3 bytes and does not divide the cap: 87376 * 3 = 262128, and the char that
    // would end at 262131 has to be dropped whole or the device decodes garbage.
    let s = "€".repeat(max); // three times the cap in bytes
    let m = mirror::set_clipboard(false, &s);
    let len = u32::from_be_bytes(m[10..14].try_into().unwrap()) as usize;
    assert_eq!(len, 262_128);
    assert_eq!(m.len(), 14 + len);
    assert!(std::str::from_utf8(&m[14..]).is_ok());
}

#[test]
fn encode_input_matches_direct_encoders() {
    let touch = InputMsg::Touch { action: 0, x: 1, y: 2, w: 10, h: 20, pressure: 1.0 };
    assert_eq!(mirror::encode_input(&touch), mirror::touch(0, 1, 2, 10, 20, 1.0).to_vec());
    let key = InputMsg::Key { action: 1, keycode: 4, meta: 0 };
    assert_eq!(mirror::encode_input(&key), mirror::keycode(1, 4, 0, 0).to_vec());
}

#[test]
fn input_msg_deserializes_from_ui_json() {
    let m: InputMsg = serde_json::from_str(
        r#"{"type":"touch","action":0,"x":10,"y":20,"w":1080,"h":2400,"pressure":1.0}"#,
    )
    .unwrap();
    assert_eq!(m, InputMsg::Touch { action: 0, x: 10, y: 20, w: 1080, h: 2400, pressure: 1.0 });
    // `meta` is optional on key events
    let m: InputMsg = serde_json::from_str(r#"{"type":"key","action":0,"keycode":66}"#).unwrap();
    assert_eq!(m, InputMsg::Key { action: 0, keycode: 66, meta: 0 });
    let m: InputMsg = serde_json::from_str(r#"{"type":"back"}"#).unwrap();
    assert_eq!(m, InputMsg::Back);
    // navigation variants are plain tags
    let m: InputMsg = serde_json::from_str(r#"{"type":"appswitch"}"#).unwrap();
    assert_eq!(m, InputMsg::AppSwitch);
    let m: InputMsg = serde_json::from_str(r#"{"type":"displaypower","on":false}"#).unwrap();
    assert_eq!(m, InputMsg::DisplayPower { on: false });
    // clipboard flags default off
    let m: InputMsg = serde_json::from_str(r#"{"type":"setclipboard","text":"x"}"#).unwrap();
    assert_eq!(m, InputMsg::SetClipboard { text: "x".into(), paste: false });
    // Reading the clipboard back is not implemented; asking is an error, not a silent `{}`.
    assert!(serde_json::from_str::<InputMsg>(r#"{"type":"getclipboard"}"#).is_err());
}

#[test]
fn input_msg_round_trips_for_the_macro_recorder() {
    // A recorder builds these from outside the module, stores them and replays them, so
    // every variant must survive serialize -> deserialize unchanged and be Clone.
    let recording = vec![
        InputMsg::Touch { action: 0, x: 1, y: 2, w: 1080, h: 2400, pressure: 1.0 },
        InputMsg::Touch { action: 2, x: 3, y: 4, w: 1080, h: 2400, pressure: 1.0 },
        InputMsg::Touch { action: 1, x: 3, y: 4, w: 1080, h: 2400, pressure: 0.0 },
        InputMsg::Scroll { x: 5, y: 6, w: 1080, h: 2400, hscroll: 0.0, vscroll: -1.0 },
        InputMsg::Key { action: 0, keycode: 66, meta: 0 },
        InputMsg::KeyPress { keycode: 66, meta: META_CTRL_ON },
        InputMsg::Text { text: "hello".into() },
        InputMsg::Back,
        InputMsg::Home,
        InputMsg::AppSwitch,
        InputMsg::Power,
        InputMsg::ScreenOn,
        InputMsg::VolumeUp,
        InputMsg::VolumeDown,
        InputMsg::VolumeMute,
        InputMsg::Rotate,
        InputMsg::Notifications,
        InputMsg::QuickSettings,
        InputMsg::Collapse,
        InputMsg::DisplayPower { on: true },
        InputMsg::SetClipboard { text: "long".into(), paste: true },
        InputMsg::ResetVideo,
    ];
    let json = serde_json::to_string(&recording.clone()).unwrap();
    let back: Vec<InputMsg> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, recording);
    // and every one of them encodes to something
    for m in &back {
        assert!(!mirror::encode_input(m).is_empty(), "{m:?} encoded to nothing");
    }
}

// -- launch line --------------------------------------------------------------

#[test]
fn the_launch_line_is_pinned() {
    // Byte for byte what the vendored jar is asked to do — it hard-fails on an unknown key.
    assert_eq!(
        mirror::server_shell_args("S", 0xabcd_1234, &mirror::ServerLaunch::new(mirror::CAPTURE_MIN, mirror::capture_bit_rate(mirror::CAPTURE_MIN))),
        vec![
            "-s",
            "S",
            "shell",
            "CLASSPATH=/data/local/tmp/scrcpy-server-relay.jar",
            "app_process",
            "/",
            "com.genymobile.scrcpy.Server",
            "4.1",
            "scid=abcd1234",
            "log_level=info",
            "audio=false",
            "video_codec=h264",
            "max_size=1024",
            "video_bit_rate=8000000",
            "max_fps=60",
            "stay_awake=true",
            "tunnel_forward=true",
        ]
    );
}

#[test]
fn the_launch_line_carries_the_refresh_cap_and_codec_options() {
    let launch = mirror::ServerLaunch {
        max_size: 1600,
        video_bit_rate: 4_000_000,
        max_fps: 120,
        video_codec_options: "vendor.qti-ext-enc-low-latency.enable:int=1".into(),
    };
    let args = mirror::server_shell_args("S", 0xabcd_1234, &launch);
    assert_eq!(
        args[args.len() - 6..],
        [
            "max_size=1600",
            // A starved rate never reaches the server, whoever asked for it.
            "video_bit_rate=8000000",
            "max_fps=120",
            "video_codec_options=vendor.qti-ext-enc-low-latency.enable:int=1",
            "stay_awake=true",
            "tunnel_forward=true",
        ]
    );
    // Out-of-range caps are clamped (0 would mean "uncapped" to the server), and a codec
    // option set that fails its check is left out rather than sent.
    let wild = mirror::ServerLaunch { max_fps: 0, video_codec_options: "a=1;reboot".into(), ..launch.clone() };
    let args = mirror::server_shell_args("S", 1, &wild);
    assert!(args.contains(&"max_fps=1".to_string()));
    assert!(!args.iter().any(|a| a.starts_with("video_codec_options")));
    let fast = mirror::ServerLaunch { max_fps: 240, video_bit_rate: 900_000_000, ..launch };
    let args = mirror::server_shell_args("S", 1, &fast);
    assert!(args.contains(&"max_fps=144".to_string()));
    assert!(args.contains(&"video_bit_rate=50000000".to_string()));
}

#[test]
fn every_rung_and_every_request_keeps_the_bit_rate_floor() {
    for rung in mirror::CAPTURE_SIZES {
        assert!(mirror::capture_bit_rate(rung) >= mirror::MIN_BIT_RATE, "rung {rung}");
    }
    assert_eq!(mirror::MIN_BIT_RATE, 8_000_000);
    assert_eq!(mirror::clamp_bit_rate(500_000), 8_000_000);
    assert_eq!(mirror::clamp_bit_rate(4_000_000), 8_000_000);
    assert_eq!(mirror::clamp_bit_rate(12_500_000), 12_500_000);
    assert_eq!(mirror::clamp_bit_rate(u32::MAX), 50_000_000);
}

// -- refresh rate from `dumpsys display` ----------------------------------------

/// The Android 16 emulator (Tally_API36), as captured: one 60 Hz mode, plus a
/// `frameRateCategoryRate … high=90.0` that is not a mode and must not count.
const DUMPSYS_EMULATOR_36: &str = r#"Display Devices: size=1
  DisplayDeviceInfo{"Built-in Screen": uniqueId="local:4619827259835644672", 1080 x 2400, modeId 1, renderFrameRate 60.000004, hasArrSupport false, frameRateCategoryRate FrameRateCategoryRate {normal=60.0, high=90.0}, supportedRefreshRates [60.000004], defaultModeId 1, userPreferredModeId -1, supportedModes [{id=1, width=1080, height=2400, fps=60.000004, vsync=60.000004, synthetic=false, alternativeRefreshRates=[], supportedHdrTypes=[]}], colorMode 0}
    mAmbientLightSensor=SensorData{type= , name= , refreshRateRange: [0.0, Infinity], supportedModes=[]}
    mBaseDisplayInfo=DisplayInfo{"Built-in Screen", displayId 0, displayGroupId 0, FLAG_SECURE, FLAG_SUPPORTS_PROTECTED_BUFFERS, FLAG_TRUSTED, real 1080 x 2400, largest app 1080 x 2400, smallest app 1080 x 2400, appVsyncOff 1000000, presDeadline 16666666, mode 1, renderFrameRate 60.000004, hasArrSupport false, frameRateCategoryRate FrameRateCategoryRate {normal=60.0, high=90.0}, supportedRefreshRates [60.000004], defaultMode 1, userPreferredModeId -1, supportedModes [{id=1, width=1080, height=2400, fps=60.000004, vsync=60.000004, synthetic=false, alternativeRefreshRates=[], supportedHdrTypes=[]}], rotation 0}
"#;

/// A Pixel on Android 14: two resolutions, each at 60 and 120, active mode 120.
const DUMPSYS_PIXEL_14: &str = r#"  DisplayDeviceInfo{"Built-in Screen": uniqueId="local:4619827551948147201", 1344 x 2992, modeId 2, renderFrameRate 120.0, defaultModeId 2, supportedModes [{id=1, width=1344, height=2992, fps=60.0, alternativeRefreshRates=[120.0], supportedHdrTypes=[2, 3, 4]}, {id=2, width=1344, height=2992, fps=120.0, alternativeRefreshRates=[60.0], supportedHdrTypes=[2, 3, 4]}], colorMode 0}
    mActiveSfDisplayMode=DisplayMode{id=1, width=1344, height=2992, xDpi=489.0, yDpi=489.0, refreshRate=120.0, appVsyncOffsetNanos=1000000, presentationDeadlineNanos=8333333, group=0}
    mBaseDisplayInfo=DisplayInfo{"Built-in Screen", displayId 0, displayGroupId 0, FLAG_SECURE, FLAG_SUPPORTS_PROTECTED_BUFFERS, FLAG_TRUSTED, real 1008 x 2244, largest app 2244 x 2244, smallest app 1008 x 1008, appVsyncOff 1000000, presDeadline 8333333, mode 4, defaultMode 4, modes [{id=1, width=1344, height=2992, fps=60.0, alternativeRefreshRates=[120.0], supportedHdrTypes=[2, 3, 4]}, {id=2, width=1344, height=2992, fps=120.0, alternativeRefreshRates=[60.0], supportedHdrTypes=[2, 3, 4]}, {id=3, width=1008, height=2244, fps=60.0, alternativeRefreshRates=[120.0], supportedHdrTypes=[2, 3, 4]}, {id=4, width=1008, height=2244, fps=120.0, alternativeRefreshRates=[60.0], supportedHdrTypes=[2, 3, 4]}], rotation 0}
"#;

/// A Samsung on Android 14, "Adaptive" motion smoothness idling: display 0's ACTIVE mode is
/// 60 Hz while it can run at 120, and a DeX monitor at 144 Hz is attached as display 2.
const DUMPSYS_SAMSUNG_ADAPTIVE_DEX: &str = r#"    mBaseDisplayInfo=DisplayInfo{"Built-in Screen", displayId 0, displayGroupId 0, FLAG_SECURE, FLAG_SUPPORTS_PROTECTED_BUFFERS, FLAG_TRUSTED, real 1080 x 2340, largest app 2340 x 2340, smallest app 1080 x 1080, appVsyncOff 1000000, presDeadline 16666666, mode 2, defaultMode 1, modes [{id=1, width=1080, height=2340, fps=120.00001, alternativeRefreshRates=[60.000004, 48.0, 30.0, 24.0, 10.0], supportedHdrTypes=[2, 3, 4]}, {id=2, width=1080, height=2340, fps=60.000004, alternativeRefreshRates=[120.00001, 48.0, 30.0, 24.0, 10.0], supportedHdrTypes=[2, 3, 4]}], rotation 0}
    mBaseDisplayInfo=DisplayInfo{"HDMI Screen", displayId 2, displayGroupId 1, real 2560 x 1440, mode 5, defaultMode 5, modes [{id=5, width=2560, height=1440, fps=144.0, alternativeRefreshRates=[], supportedHdrTypes=[]}], rotation 0}
"#;

/// Android 10: `displayId 0` sits inside the quoted name, and modes carry only `fps=`.
const DUMPSYS_ANDROID_10: &str = r#"  mBaseDisplayInfo=DisplayInfo{"Built-in Screen, displayId 0", uniqueId "local:0", app 1080 x 2280, real 1080 x 2280, largest app 2280 x 2209, smallest app 1080 x 1009, mode 2, defaultMode 1, modes [{id=1, width=1080, height=2280, fps=60.000004}, {id=2, width=1080, height=2280, fps=90.0}], colorMode 0}
  mBaseDisplayInfo=DisplayInfo{"Overlay #1, displayId 01", app 720 x 480, mode 9, modes [{id=9, width=720, height=480, fps=144.0}]}
"#;

#[test]
fn the_refresh_rate_is_display_zeros_top_mode_across_formats() {
    let rate = |text: &str| mirror::display_refresh_rate(text).map(|rate| (rate * 100.0).round() / 100.0);
    assert_eq!(rate(DUMPSYS_EMULATOR_36), Some(60.0));
    assert_eq!(rate(DUMPSYS_PIXEL_14), Some(120.0));
    // The top mode, not the active one; display 0, not the faster monitor beside it.
    assert_eq!(rate(DUMPSYS_SAMSUNG_ADAPTIVE_DEX), Some(120.0));
    assert_eq!(rate(DUMPSYS_ANDROID_10), Some(90.0));
    // Only SurfaceFlinger's spelling survives: still read.
    assert_eq!(rate("mActiveSfDisplayMode=DisplayMode{id=0, width=1080, height=2400, refreshRate=90.0, group=0}"), Some(90.0));
    // Nothing usable: `alternativeRefreshRates=` and `refreshRateRange` are not rates.
    assert_eq!(rate(""), None);
    assert_eq!(rate("error: closed"), None);
    assert_eq!(rate("alternativeRefreshRates=[120.0] refreshRateRange: [0.0, Infinity] xfps=200"), None);
}

#[test]
fn max_fps_follows_the_panel_within_bounds() {
    assert_eq!(mirror::max_fps_for(None), 60);
    assert_eq!(mirror::max_fps_for(Some(60.000004)), 60);
    assert_eq!(mirror::max_fps_for(Some(59.94)), 60);
    assert_eq!(mirror::max_fps_for(Some(90.0)), 90);
    assert_eq!(mirror::max_fps_for(Some(120.00001)), 120);
    assert_eq!(mirror::max_fps_for(Some(144.0)), 144);
    // Never under today's 60, never past the cap.
    assert_eq!(mirror::max_fps_for(Some(30.0)), 60);
    assert_eq!(mirror::max_fps_for(Some(165.0)), 144);
}

// -- video_codec_options ----------------------------------------------------------

#[test]
fn codec_options_merge_over_the_base_by_key() {
    // A user's key replaces the base's in place; new keys follow in the order given.
    assert_eq!(
        mirror::merge_codec_options("latency=1,priority=0", "priority:int=1, vendor.qti-ext-enc-low-latency.enable:int=1,").unwrap(),
        "latency=1,priority:int=1,vendor.qti-ext-enc-low-latency.enable:int=1"
    );
    assert_eq!(mirror::merge_codec_options("", "").unwrap(), "");
    assert_eq!(mirror::merge_codec_options(mirror::BASE_CODEC_OPTIONS, "i-frame-interval:float=0.5").unwrap(), "i-frame-interval:float=0.5");
    // A key given twice is one entry: the last wins.
    assert_eq!(mirror::merge_codec_options("", "a=1,a=2").unwrap(), "a=2");
    assert_eq!(mirror::merge_codec_options("", "profile:string=high,bitrate-mode:long=-1").unwrap(), "profile:string=high,bitrate-mode:long=-1");
}

#[test]
fn codec_options_refuse_what_the_server_or_the_device_shell_would_misread() {
    for bad in [
        "latency",                 // no value
        "latency:bool=1",          // a type the server does not know
        "latency:int=fast",        // does not parse as its type
        "latency=99999999999",     // past an int
        "a=1;reboot",              // the line goes through `adb shell`
        "a=$(id)",
        "a b=1",
        "=1",
        "a:string=two words",
    ] {
        assert!(mirror::merge_codec_options("", bad).is_err(), "{bad:?} was accepted");
        // One bad entry refuses the whole value.
        assert!(mirror::merge_codec_options("", &format!("ok=1,{bad}")).is_err(), "{bad:?} was accepted");
    }
}

// -- trace -------------------------------------------------------------------

#[test]
fn trace_ids_come_off_the_head_of_a_serialized_video_frame() {
    let frame = |seq: u64, data: serde_json::Value| {
        serde_json::to_string(&relay_bus::envelope::Frame {
            v: 1,
            stream: "mirror".into(),
            session: None,
            run_id: None,
            mirror_id: Some(7),
            epoch: None,
            seq,
            data,
        })
        .unwrap()
    };
    assert_eq!(mirror::trace_frame_ids(&frame(42, serde_json::Value::String("AAAA".repeat(100)))), Some((7, 42)));
    // Status frames go out at seq 0 and are not packets.
    assert_eq!(mirror::trace_frame_ids(&frame(0, serde_json::json!({"state":"running"}))), None);
    assert_eq!(mirror::trace_frame_ids(r#"{"id":1,"ok":true,"result":{}}"#), None);
    assert!(mirror::monotonic_ns() > 0);
}

// -- runtime: picture size, status, input fast path, worker end -------------
//
// The handoff that motivated these: a window sat on "Starting mirror…" or a frozen frame
// forever because no failure ever reached it; tap/swipe were checked against a width the
// stream did not have; and every touch-move took the store mutex.

mod runtime {
    use relay_bus::{Actor, Request};
    use relay_core::device::{MirrorRuntime, MirrorRuntimeConfig, MirrorState};
    use relay_core::mirror;
    use relay_core::{Door, Engine, Instance, Store};
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn engine() -> Arc<Engine> {
        Engine::new(Instance::Test, Store::open_memory().unwrap())
    }

    #[allow(clippy::result_large_err)] // the bus's own error type, as `into_result` returns it
    fn call(e: &Engine, op: &str, payload: Value) -> Result<Value, relay_bus::error::BusError> {
        e.dispatch(Request::new(Actor::User, op, payload), Door::InProcess).into_result()
    }

    /// A fake adb: one device, `wm size` 1080x2400, and enough of push/forward/shell for the
    /// worker. `forward` prints `$port`; the server "process" just sleeps, because the test
    /// itself plays the server on that port. `get-state` answers from `state`.
    ///
    /// `dumpsys display` prints `dir/dumpsys` when there is one; `settings` keeps the device's
    /// `min_refresh_rate` in `dir/min_refresh_rate` (absent = unset) and logs each call to
    /// `dir/settings-log`; the server launch line is written to `dir/server-args`.
    fn fake_adb(dir: &std::path::Path, port: u16, state: &str) -> String {
        let path = dir.join("adb");
        let d = dir.display();
        std::fs::write(&path, format!(r#"#!/bin/sh
[ "$1" = "-s" ] && shift 2
case "$1" in
  devices) printf 'List of devices attached\nrelay-phone device product:relay model:Pixel_9_Pro device:relay transport_id:1\n' ;;
  push) exit 0 ;;
  get-state) [ "{state}" = device ] && echo device && exit 0; echo "error: device not found" >&2; exit 1 ;;
  forward) [ "$2" = "--remove" ] && exit 0; echo {port} ;;
  shell)
    [ "$2" = wm ] && echo 'Physical size: 1080x2400' && exit 0
    if [ "$2" = dumpsys ]; then [ -f "{d}/dumpsys" ] && cat "{d}/dumpsys"; exit 0; fi
    if [ "$2" = settings ]; then
      echo "$*" >> "{d}/settings-log"
      case "$3" in
        get) if [ -f "{d}/min_refresh_rate" ]; then cat "{d}/min_refresh_rate"; else echo null; fi ;;
        put) echo "$6" > "{d}/min_refresh_rate" ;;
        delete) rm -f "{d}/min_refresh_rate" ;;
      esac
      exit 0
    fi
    echo "$*" > "{d}/server-args"
    exec sleep 30 ;;
  *) exit 1 ;;
esac
"#)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.display().to_string()
    }

    fn runtime(width: u32, height: u32) -> Arc<MirrorRuntime> {
        MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1,
            device: "relay-phone".into(),
            width,
            height,
            max_size: 1024,
            bitrate: 8_000_000,
            scid: 1,
            adb: "adb".into(),
        })
    }

    /// A control socket the test can read back: what the device would receive.
    fn control_pair(runtime: &MirrorRuntime) -> TcpStream {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (device, _) = listener.accept().unwrap();
        runtime.install_control(client).unwrap();
        device.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        device
    }

    #[test]
    fn fit_size_rounds_the_short_side_to_eight_like_the_server() {
        // The old handler rounded to 2 and said 460; the server sends 464 for this phone.
        assert_eq!(mirror::fit_size(1080, 2400, 1024), (464, 1024));
        assert_eq!(mirror::fit_size(2400, 1080, 1024), (1024, 464));
        assert_eq!(mirror::fit_size(1080, 2400, 1280), (576, 1280));
        // Under the cap nothing scales, but both sides still land on a multiple of 8.
        assert_eq!(mirror::fit_size(1001, 803, 1024), (1000, 800));
        assert_eq!(mirror::fit_size(0, 0, 1024), (0, 0));
        for (w, h, max) in [(1440, 3120, 1600), (720, 1600, 1024), (1200, 1920, 1920)] {
            let (a, b) = mirror::fit_size(w, h, max);
            assert_eq!((a % 8, b % 8), (0, 0), "{w}x{h}@{max}");
            assert!(a.max(b) <= max);
        }
    }

    #[test]
    fn session_packets_move_the_size_tap_and_swipe_are_checked_against() {
        // Started from the `wm size` estimate; the device then rotates to landscape.
        let r = runtime(464, 1024);
        let mut device = control_pair(&r);
        r.set_size(1024, 464);
        assert_eq!(r.size(), (1024, 464));
        // x=900 is outside the portrait estimate, and valid in landscape.
        relay_core::handlers::device::send_input(&r, &json!({"type":"tap","x":900,"y":400})).unwrap();
        let mut bytes = [0u8; 64];
        device.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes[..32], &mirror::touch(mirror::ACTION_DOWN, 900, 400, 1024, 464, 1.0));
        assert_eq!(&bytes[32..], &mirror::touch(mirror::ACTION_UP, 900, 400, 1024, 464, 0.0));
        // y=600 was fine in portrait and is now off the picture: refused, not sent to be
        // dropped silently by the server.
        let err = relay_core::handlers::device::send_input(&r, &json!({"type":"tap","x":10,"y":600})).unwrap_err();
        assert_eq!(err.code, "device.input");
        // Status carries the new size while live, so the window can re-letterbox.
        r.set_running("Pixel".into());
        r.set_size(464, 1024);
        let status = r.status();
        assert_eq!((status.state, status.width, status.height), (MirrorState::Running, 464, 1024));
    }

    #[test]
    fn the_first_terminal_state_wins() {
        let r = runtime(464, 1024);
        assert_eq!(r.status().state, MirrorState::Starting);
        r.set_running("Pixel".into());
        assert!(r.finish(MirrorState::Lost, Some("device.mirror_device_lost".into()), Some("gone".into())));
        // A stop racing the loss must not relabel it — the window would say "Stopped" for an
        // unplugged phone.
        assert!(!r.finish(MirrorState::Stopped, None, None));
        r.set_running("again".into());
        r.set_size(1, 1);
        let status = r.status();
        assert_eq!(status.state, MirrorState::Lost);
        assert_eq!(status.code.as_deref(), Some("device.mirror_device_lost"));
        assert_eq!((status.width, status.height), (464, 1024));
        let wire = serde_json::to_value(&status).unwrap();
        assert_eq!(wire["state"], "lost");
        assert_eq!(wire["message"], "gone");
    }

    #[test]
    fn mirror_input_is_answered_without_the_store() {
        let e = engine();
        let dir = tempfile::tempdir().unwrap();
        let adb = fake_adb(dir.path(), 1, "device");
        call(&e, "settings.set", json!({"path":"device.adb_path","value":adb})).unwrap();
        let started = call(&e, "device.mirror.start", json!({"device":"relay-phone","max_size":1024})).unwrap();
        assert_eq!((started["width"].as_u64(), started["height"].as_u64()), (Some(464), Some(1024)));
        let id = started["mirror_id"].as_i64().unwrap();

        let touch = |id: i64| Request::new(
            Actor::User,
            "device.mirror.input",
            json!({"mirror_id":id,"event":{"type":"touch","action":2,"x":10,"y":10,"w":464,"h":1024,"pressure":1.0}}),
        );
        assert!(e.answers_from_memory(&touch(id)));
        assert!(!e.answers_from_memory(&touch(id + 99)));
        // Text can be big enough to block on the socket, so it keeps to the blocking pool.
        assert!(!e.answers_from_memory(&Request::new(
            Actor::User,
            "device.mirror.input",
            json!({"mirror_id":id,"event":{"type":"text","text":"hello"}}),
        )));

        // Hold the store for the whole exchange. A request that needs it would block here
        // until the guard drops; the fast path answers anyway.
        let guard = e.store.lock();
        let (done, result) = std::sync::mpsc::channel();
        let engine = e.clone();
        std::thread::spawn(move || {
            let ok = engine.dispatch(touch(id), Door::Socket).into_result();
            let missing = engine.dispatch(touch(id + 99), Door::Socket).into_result();
            let _ = done.send((ok, missing));
        });
        let (ok, missing) = result.recv_timeout(Duration::from_secs(5)).expect("mirror input waited for the store");
        drop(guard);
        ok.unwrap();
        assert_eq!(missing.unwrap_err().code, "device.mirror_not_found");
        call(&e, "device.mirror.stop", json!({"mirror_id":id})).unwrap();
    }

    /// Accept on a non-blocking listener until `stop` is set. The worker can end without ever
    /// connecting (a push or forward the fake adb does not know), and a blocking accept would
    /// then hang the test instead of failing it.
    fn accept_until(listener: &TcpListener, stop: &AtomicBool) -> Option<TcpStream> {
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    return Some(stream);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if stop.load(Ordering::SeqCst) {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("accept: {e}"),
            }
        }
    }

    /// Play the scrcpy server on `listener`: handshake, one session packet, a config and a key
    /// frame, then drop both sockets the way an unplugged phone does. Returns whether the
    /// worker ever connected; gives up waiting for it once `stop` is set.
    fn serve_then_vanish(listener: TcpListener, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<bool> {
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            let Some(mut video) = accept_until(&listener, &stop) else { return false };
            video.write_all(&[0]).unwrap();
            let Some(control) = accept_until(&listener, &stop) else { return false };
            let mut name = [0u8; 64];
            name[..5].copy_from_slice(b"Pixel");
            video.write_all(&name).unwrap();
            video.write_all(b"h264").unwrap();
            let mut session = [0u8; 12];
            session[0] = 0x80;
            session[4..8].copy_from_slice(&1024u32.to_be_bytes());
            session[8..12].copy_from_slice(&464u32.to_be_bytes());
            video.write_all(&session).unwrap();
            for (flags, payload) in [(1u64 << 62, &[0, 0, 0, 1, 0x67][..]), (1u64 << 61, &[0, 0, 0, 1, 0x65][..])] {
                video.write_all(&flags.to_be_bytes()).unwrap();
                video.write_all(&(payload.len() as u32).to_be_bytes()).unwrap();
                video.write_all(payload).unwrap();
            }
            std::thread::sleep(Duration::from_millis(200));
            drop(control);
            drop(video);
            true
        })
    }

    fn run_worker_until_it_ends(get_state: &str) -> (Arc<Engine>, i64, relay_core::device::MirrorStatus, Vec<Value>) {
        let dir = tempfile::tempdir().unwrap();
        let ended = run_worker_in(engine(), dir.path(), get_state);
        // The launch line the worker built: the fixture's 120 Hz panel sets the cap, and the
        // ladder's rate for 1024 is the 8 Mbps floor.
        let args = std::fs::read_to_string(dir.path().join("server-args")).unwrap();
        assert!(args.contains(" max_fps=120 "), "{args}");
        assert!(args.contains(" video_bit_rate=8000000 "), "{args}");
        assert!(!args.contains("video_codec_options"), "{args}");
        ended
    }

    /// A Samsung-style `dumpsys display`: display 0 idling in its 60 Hz mode, a 120 Hz mode
    /// beside it.
    const DUMPSYS_120: &str = "    mBaseDisplayInfo=DisplayInfo{\"Built-in Screen\", displayId 0, displayGroupId 0, real 1080 x 2400, mode 2, defaultMode 1, modes [{id=1, width=1080, height=2400, fps=120.00001, alternativeRefreshRates=[60.000004]}, {id=2, width=1080, height=2400, fps=60.000004, alternativeRefreshRates=[120.00001]}], rotation 0}\n";

    fn run_worker_in(e: Arc<Engine>, dir: &std::path::Path, get_state: &str) -> (Arc<Engine>, i64, relay_core::device::MirrorStatus, Vec<Value>) {
        std::fs::write(dir.join("dumpsys"), DUMPSYS_120).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // The source-tree jar is the server path in tests; the fake adb "pushes" it anywhere.
        assert!(relay_core::device::mirror_server().is_ok());
        let adb = fake_adb(dir, port, get_state);
        call(&e, "settings.set", json!({"path":"device.adb_path","value":adb})).unwrap();
        let mut events = e.subscribe();
        let started = call(&e, "device.mirror.start", json!({"device":"relay-phone","max_size":1024})).unwrap();
        let id = started["mirror_id"].as_i64().unwrap();
        let runtime = relay_core::handlers::device::mirror_by_id(&e, id).unwrap();
        let mut status = runtime.watch_status();
        let stop = Arc::new(AtomicBool::new(false));
        let server = serve_then_vanish(listener, stop.clone());
        let worker = {
            let (e, r) = (e.clone(), runtime.clone());
            std::thread::spawn(move || relay_core::handlers::device::mirror_worker(e, r))
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        while !status.borrow_and_update().state.is_terminal() {
            assert!(Instant::now() < deadline, "worker never reported an end: {:?}", runtime.status());
            std::thread::sleep(Duration::from_millis(20));
        }
        stop.store(true, Ordering::SeqCst);
        worker.join().unwrap();
        assert!(
            matches!(server.join(), Ok(true)),
            "worker ended before it was served: {:?}",
            runtime.status()
        );
        // The session packet set the size before "running" was announced.
        assert_eq!(runtime.size(), (1024, 464));
        let mut seen = Vec::new();
        while let Ok(event) = events.try_recv() {
            if event.ev == "mirror.changed" {
                seen.push(event.payload);
            }
        }
        (e, id, runtime.status(), seen)
    }

    #[test]
    fn a_vanished_device_ends_the_mirror_as_lost_and_says_so() {
        let (e, id, status, events) = run_worker_until_it_ends("gone");
        assert_eq!(status.state, MirrorState::Lost);
        assert_eq!(status.code.as_deref(), Some("device.mirror_device_lost"));
        assert!(status.message.unwrap().contains("relay-phone"));
        assert_eq!(status.name.as_deref(), Some("Pixel"));
        let running = events.iter().find(|ev| ev["state"] == "running").expect("running event");
        assert_eq!((running["width"].as_u64(), running["height"].as_u64()), (Some(1024), Some(464)));
        assert_eq!(running["name"], "Pixel");
        assert!(events.iter().any(|ev| ev["state"] == "lost" && ev["code"] == "device.mirror_device_lost"));
        assert!(relay_core::handlers::device::mirror_by_id(&e, id).is_err(), "an ended mirror leaves the registry");
    }

    #[test]
    fn a_dead_server_on_a_present_device_is_a_failure_with_the_reason() {
        let (_, _, status, events) = run_worker_until_it_ends("device");
        assert_eq!(status.state, MirrorState::Failed);
        assert_eq!(status.code.as_deref(), Some("device.mirror_stream_failed"));
        assert!(status.message.unwrap().contains("mirror stream ended"));
        assert!(events.iter().any(|ev| ev["state"] == "failed"));
    }

    fn file_engine(dir: &std::path::Path) -> Arc<Engine> {
        Engine::new(Instance::Test, Store::open(&dir.join("store.db"), false).unwrap())
    }

    fn device_setting(dir: &std::path::Path) -> Option<String> {
        std::fs::read_to_string(dir.join("min_refresh_rate")).ok().map(|value| value.trim().to_string())
    }

    #[test]
    fn a_pinned_refresh_rate_is_put_back_when_the_mirror_ends() {
        let dir = tempfile::tempdir().unwrap();
        let e = file_engine(dir.path());
        call(&e, "settings.set", json!({"path":"device.mirror.pin_refresh_rate","value":true})).unwrap();
        std::fs::write(dir.path().join("min_refresh_rate"), "48.0\n").unwrap();
        run_worker_in(e, dir.path(), "device");
        let log = std::fs::read_to_string(dir.path().join("settings-log")).unwrap();
        // Read first, pinned at the panel's top rate, read back to check, and finally the
        // previous value put back — and the record of it gone.
        let calls: Vec<&str> = log.lines().collect();
        assert_eq!(
            calls,
            [
                "shell settings get system min_refresh_rate",
                "shell settings put system min_refresh_rate 120",
                "shell settings get system min_refresh_rate",
                "shell settings put system min_refresh_rate 48.0",
                "shell settings get system min_refresh_rate",
            ],
            "{log}"
        );
        assert_eq!(device_setting(dir.path()).as_deref(), Some("48.0"));
        assert!(!dir.path().join("mirror-refresh").exists());
    }

    #[test]
    fn a_pin_a_crashed_run_left_is_restored_to_what_was_there_before_it() {
        // The last run pinned an unset setting and died: the device says 120, the record says
        // it was unset. A new pin must not mistake 120 for the user's value.
        let dir = tempfile::tempdir().unwrap();
        let e = file_engine(dir.path());
        call(&e, "settings.set", json!({"path":"device.mirror.pin_refresh_rate","value":true})).unwrap();
        std::fs::write(dir.path().join("min_refresh_rate"), "120\n").unwrap();
        std::fs::write(dir.path().join("mirror-refresh"), "relay-phone null\n").unwrap();
        run_worker_in(e, dir.path(), "device");
        assert_eq!(device_setting(dir.path()), None, "restored to unset, not to the stale pin");
        assert!(!dir.path().join("mirror-refresh").exists());
        let log = std::fs::read_to_string(dir.path().join("settings-log")).unwrap();
        assert!(log.lines().any(|line| line == "shell settings delete system min_refresh_rate"), "{log}");
    }

    #[test]
    fn without_the_setting_a_left_over_pin_is_put_back_and_nothing_new_is_pinned() {
        let dir = tempfile::tempdir().unwrap();
        let e = file_engine(dir.path());
        std::fs::write(dir.path().join("min_refresh_rate"), "120\n").unwrap();
        std::fs::write(dir.path().join("mirror-refresh"), "relay-phone 60.0\n").unwrap();
        run_worker_in(e, dir.path(), "device");
        assert_eq!(device_setting(dir.path()).as_deref(), Some("60.0"));
        assert!(!dir.path().join("mirror-refresh").exists());
        let log = std::fs::read_to_string(dir.path().join("settings-log")).unwrap();
        assert!(!log.contains("put system min_refresh_rate 120"), "{log}");
    }

    #[test]
    fn an_in_memory_engine_never_pins() {
        // Nowhere to remember the previous value across a crash: no pin at all.
        let dir = tempfile::tempdir().unwrap();
        let e = engine();
        call(&e, "settings.set", json!({"path":"device.mirror.pin_refresh_rate","value":true})).unwrap();
        run_worker_in(e, dir.path(), "device");
        assert!(!dir.path().join("settings-log").exists());
    }

    #[test]
    fn settings_override_the_cap_and_add_codec_options() {
        let dir = tempfile::tempdir().unwrap();
        let e = engine();
        call(&e, "settings.set", json!({"path":"device.mirror","value":{"max_fps":60,"video_codec_options":"vendor.qti-ext-enc-low-latency.enable:int=1"}})).unwrap();
        run_worker_in(e, dir.path(), "device");
        let args = std::fs::read_to_string(dir.path().join("server-args")).unwrap();
        assert!(args.contains(" max_fps=60 "), "{args}");
        assert!(args.contains(" video_codec_options=vendor.qti-ext-enc-low-latency.enable:int=1 "), "{args}");
        // A value the device shell would interpret is dropped whole; the mirror still starts.
        let dir = tempfile::tempdir().unwrap();
        let e = engine();
        call(&e, "settings.set", json!({"path":"device.mirror.video_codec_options","value":"latency=1;reboot"})).unwrap();
        run_worker_in(e, dir.path(), "device");
        let args = std::fs::read_to_string(dir.path().join("server-args")).unwrap();
        assert!(!args.contains("video_codec_options") && !args.contains("reboot"), "{args}");
    }

    /// Against a real device: `RELAY_MIRROR_LIVE_SERIAL=<serial> cargo test -p relay-core --test
    /// mirror -- --ignored live_mirror --nocapture` (adb from PATH). Starts a mirror the way the
    /// engine does, prints the server's command line as the device runs it, and checks the frame
    /// cap and bit rate made it there. `RELAY_MIRROR_LIVE_PIN=1` also pins the refresh rate, and
    /// checks the device's `min_refresh_rate` is back where it was after the stop.
    #[test]
    #[ignore = "needs a device: RELAY_MIRROR_LIVE_SERIAL"]
    fn live_mirror_launch() {
        let Ok(serial) = std::env::var("RELAY_MIRROR_LIVE_SERIAL") else { return };
        let pin = std::env::var("RELAY_MIRROR_LIVE_PIN").is_ok_and(|value| value == "1");
        let adb = which::which("adb").unwrap().display().to_string();
        let shell = |args: &[&str]| {
            let out = std::process::Command::new(&adb).args(["-s", &serial, "shell"]).args(args).output().unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let before = shell(&["settings", "get", "system", "min_refresh_rate"]);
        let dir = tempfile::tempdir().unwrap();
        let e = file_engine(dir.path());
        call(&e, "settings.set", json!({"path":"device.adb_path","value":adb})).unwrap();
        call(&e, "settings.set", json!({"path":"device.mirror.pin_refresh_rate","value":pin})).unwrap();
        // Optional overrides, to see them reach the device: RELAY_MIRROR_LIVE_MAX_FPS and
        // RELAY_MIRROR_LIVE_CODEC (a `video_codec_options` value).
        if let Some(fps) = std::env::var("RELAY_MIRROR_LIVE_MAX_FPS").ok().and_then(|fps| fps.parse::<u64>().ok()) {
            call(&e, "settings.set", json!({"path":"device.mirror.max_fps","value":fps})).unwrap();
        }
        if let Ok(options) = std::env::var("RELAY_MIRROR_LIVE_CODEC") {
            call(&e, "settings.set", json!({"path":"device.mirror.video_codec_options","value":options})).unwrap();
        }
        let runtime = MirrorRuntime::new(MirrorRuntimeConfig {
            id: 1,
            device: serial.clone(),
            width: 0,
            height: 0,
            max_size: 1024,
            bitrate: mirror::capture_bit_rate(1024),
            scid: 0x0bad_cafe,
            adb: adb.clone(),
        });
        let packets = runtime.attach();
        let worker = {
            let (e, r) = (e.clone(), runtime.clone());
            std::thread::spawn(move || relay_core::handlers::device::mirror_worker(e, r))
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        while runtime.status().state != MirrorState::Running {
            assert!(Instant::now() < deadline && !runtime.status().state.is_terminal(), "{:?}", runtime.status());
            std::thread::sleep(Duration::from_millis(50));
        }
        let server = shell(&["ps", "-A", "-o", "ARGS"]).lines().find(|line| line.contains("scid=0badcafe")).unwrap_or_default().to_string();
        let during = shell(&["settings", "get", "system", "min_refresh_rate"]);
        println!("device runs: {server}");
        println!("min_refresh_rate: before {before}, during {during}");
        std::thread::sleep(Duration::from_secs(2));
        println!("packets in the first ~2 s: {}", packets.len());
        runtime.stop();
        worker.join().unwrap();
        let after = shell(&["settings", "get", "system", "min_refresh_rate"]);
        println!("min_refresh_rate after: {after}");
        assert!(server.contains("max_fps="), "{server}");
        assert!(server.contains("video_bit_rate=8000000"), "{server}");
        assert_eq!(after, before, "the device's setting is back");
        assert!(!dir.path().join("mirror-refresh").exists());
    }

    async fn next_frame(client: &mut relay_core::socket::Client, id: i64) -> relay_bus::envelope::Frame {
        loop {
            match tokio::time::timeout(Duration::from_secs(2), client.next()).await.unwrap().unwrap().unwrap() {
                relay_core::socket::Line::Frame(frame) => {
                    assert_eq!(frame.stream, "mirror");
                    assert_eq!(frame.mirror_id, Some(id));
                    return frame;
                }
                _ => continue,
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_window_stream_carries_status_and_ends_on_a_terminal_state() {
        use relay_core::socket::{Client, SocketServer};
        let e = engine();
        let dir = tempfile::tempdir().unwrap();
        let adb = fake_adb(dir.path(), 1, "device");
        call(&e, "settings.set", json!({"path":"device.adb_path","value":adb})).unwrap();
        let server = SocketServer::start_in(e.clone(), dir.path().join("socket")).await.unwrap();
        let mut client = Client::connect(&server.path).await.unwrap();
        let started = client
            .call(&Request::new(Actor::User, "device.mirror.start", json!({"device":"relay-phone"})), |_| {})
            .await
            .unwrap()
            .into_result()
            .unwrap();
        let id = started["mirror_id"].as_i64().unwrap();
        let runtime = relay_core::handlers::device::mirror_by_id(&e, id).unwrap();
        let first = next_frame(&mut client, id).await;
        assert_eq!(first.data["state"], "starting");
        assert_eq!(first.data["width"], 464);
        runtime.set_running("Pixel".into());
        let running = next_frame(&mut client, id).await;
        assert_eq!(running.data["state"], "running");
        assert_eq!(running.data["name"], "Pixel");
        runtime.push(vec![1, 0, 0, 0, 1, 0x67]);
        assert!(next_frame(&mut client, id).await.data.is_string(), "video packets are base64 strings");
        runtime.finish(MirrorState::Lost, Some("device.mirror_device_lost".into()), Some("relay-phone is no longer connected".into()));
        let lost = next_frame(&mut client, id).await;
        assert_eq!(lost.data["state"], "lost");
        assert_eq!(lost.data["code"], "device.mirror_device_lost");
        // Nothing follows a terminal state on this mirror's stream.
        runtime.push(vec![2, 0, 0, 0, 1, 0x65]);
        assert!(tokio::time::timeout(Duration::from_millis(300), client.next()).await.is_err());
        drop(client);
    }
}
