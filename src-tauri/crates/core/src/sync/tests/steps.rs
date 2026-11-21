//! Sync tests for steps: the capability handshake, per-step merge rules,
//! holding for unknown notes, and three-device convergence.

use std::collections::VecDeque;

use crate::sync::{decode_frame, encode_frame, Message, Session, CAP_STEPS, PROTOCOL_VERSION};
use crate::{Db, NewNote};

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        category: None,
        source: None,
    }
}

fn setup_pair() -> (Db, Db) {
    let mut a = Db::open_in_memory().unwrap();
    let mut b = Db::open_in_memory().unwrap();
    a.set_sync_device_id("dev-a").unwrap();
    b.set_sync_device_id("dev-b").unwrap();
    a.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
    b.upsert_peer("dev-a", "A", &[], 1_000).unwrap();
    (a, b)
}

fn note_with_steps(db: &mut Db, how_many: usize) -> String {
    let n = db
        .create_note_at(
            NewNote {
                content: "a note".into(),
                category: None,
                source: None,
            },
            1_000,
        )
        .unwrap();
    let lines: Vec<crate::NewStep> = (0..how_many)
        .map(|i| crate::NewStep {
            text: format!("step {i}"),
            done: false,
        })
        .collect();
    db.add_steps_at(&n.id, None, &lines, 1_100).unwrap();
    n.id
}

/// One export/apply round trip, the way a session does it.
fn sync_one_way(src: &mut Db, src_id: &str, dst: &mut Db, dst_id: &str, now_ms: i64) {
    let since = src
        .get_peer(dst_id)
        .unwrap()
        .map(|p| p.sent_up_to)
        .unwrap_or(0);
    let export = src.changes_since(since, 1_000, true).unwrap();
    if !export.changes.is_empty() || !export.steps.is_empty() {
        dst.apply_changes(src_id, &export.changes, &export.steps, now_ms)
            .unwrap();
        src.update_peer_sent_up_to(dst_id, export.up_to, now_ms)
            .unwrap();
    }
}

fn sync_both_ways(a: &mut Db, b: &mut Db, now_ms: i64) {
    sync_one_way(a, "dev-a", b, "dev-b", now_ms);
    sync_one_way(b, "dev-b", a, "dev-a", now_ms);
}

fn steps_text(db: &Db, note_id: &str) -> Vec<(String, bool)> {
    db.list_steps(note_id)
        .unwrap()
        .into_iter()
        .map(|s| (s.text, s.done))
        .collect()
}

// ---- the handshake is the release gate ---------------------------------------

#[test]
fn an_old_hello_without_capabilities_still_parses() {
    // A frame exactly as a pre-steps build writes it: no capabilities key.
    let old = br#"{"type":"hello","protocol":1,"device_id":"dev-b","name":"B","pair_secret":null}"#;
    let mut frame = (old.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(old);
    match decode_frame(&frame).unwrap() {
        Message::Hello {
            protocol,
            device_id,
            capabilities,
            ..
        } => {
            assert_eq!(protocol, 1);
            assert_eq!(device_id, "dev-b");
            assert_eq!(capabilities, Vec::<String>::new());
        }
        other => panic!("expected hello, got {other:?}"),
    }
}

#[test]
fn an_old_build_can_read_a_hello_with_capabilities() {
    // The v1 message set, without the capabilities field, must decode a
    // frame that carries it: serde ignores the unknown key.
    #[derive(Debug, serde::Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum V1Message {
        Hello {
            protocol: u32,
            device_id: String,
            name: String,
            pair_secret: Option<String>,
        },
        #[serde(other)]
        Other,
    }
    let hello = Message::Hello {
        protocol: PROTOCOL_VERSION,
        device_id: "dev-b".into(),
        name: "B".into(),
        pair_secret: None,
        capabilities: vec![CAP_STEPS.to_owned()],
    };
    let frame = encode_frame(&hello).unwrap();
    let v1: V1Message = serde_json::from_slice(&frame[4..]).unwrap();
    match v1 {
        V1Message::Hello { device_id, .. } => assert_eq!(device_id, "dev-b"),
        _ => panic!("the hello must still decode on an old build"),
    }
}

/// A session pair where B presents the hello of a pre-steps build.
fn hello_without_capabilities(device_id: &str, name: &str) -> Message {
    Message::Hello {
        protocol: PROTOCOL_VERSION,
        device_id: device_id.into(),
        name: name.into(),
        pair_secret: None,
        capabilities: Vec::new(),
    }
}

#[test]
fn an_old_build_never_receives_step_fields_and_ordinary_notes_still_sync() {
    let (mut a, mut b) = setup_pair();
    let n = a.create_note_at(note("shared note"), 1_000).unwrap();
    note_with_steps(&mut a, 2);

    // A announces steps; B answers with an old build's hello.
    let mut sa = Session::new("dev-a", "dev-b");
    let replies = sa
        .on_message(&mut a, hello_without_capabilities("dev-b", "B"), 2_000)
        .unwrap();
    // The batch A produced carries ordinary notes and no step changes.
    let (changes, steps) = replies
        .into_iter()
        .find_map(|msg| match msg {
            Message::Batch { changes, steps, .. } => Some((changes, steps)),
            _ => None,
        })
        .expect("A sends its first batch");
    assert!(!changes.is_empty(), "ordinary notes still sync");
    assert!(steps.is_empty(), "an old peer never sees steps");

    // The old build applies what it got, as it would: the note arrives.
    b.apply_changes("dev-a", &changes, &[], 3_000).unwrap();
    assert_eq!(b.get_note(&n.id).unwrap().content, "shared note");
    assert_eq!(
        b.list_steps(&n.id).unwrap(),
        [] as [crate::Step; 0],
        "no steps reached the old build"
    );
}

/// A hello from a build that announces the steps capability.
fn hello_with_capabilities(device_id: &str, name: &str) -> Message {
    Message::Hello {
        protocol: PROTOCOL_VERSION,
        device_id: device_id.into(),
        name: name.into(),
        pair_secret: None,
        capabilities: vec![CAP_STEPS.to_owned()],
    }
}

/// Feeds every batch among `replies` to `dst`, answering each with an Ack,
/// and returns how many step changes were applied.
fn drain_batches(
    src: &mut Db,
    sa: &mut Session,
    dst: &mut Db,
    now_ms: i64,
    replies: Vec<Message>,
) -> usize {
    let mut applied = 0;
    let mut replies = replies;
    loop {
        let mut next = Vec::new();
        for msg in replies {
            if let Message::Batch {
                changes,
                steps,
                up_to,
                ..
            } = msg
            {
                applied += steps.len();
                dst.apply_changes("dev-a", &changes, &steps, now_ms)
                    .unwrap();
                next.extend(sa.on_message(src, Message::Ack { up_to }, now_ms).unwrap());
            }
        }
        if next.is_empty() || sa.is_finished() {
            break;
        }
        replies = next;
    }
    applied
}

#[test]
fn step_changes_withheld_from_an_old_peer_are_all_sent_after_it_upgrades() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 3);
    a.remove_step(&a.list_steps(&n).unwrap()[0].id).unwrap();

    // The old peer gets ordinary notes; every step change is withheld.
    let mut sa = Session::new("dev-a", "dev-b");
    let replies = sa
        .on_message(&mut a, hello_without_capabilities("dev-b", "B"), 2_000)
        .unwrap();
    let (changes, steps) = replies
        .into_iter()
        .find_map(|msg| match msg {
            Message::Batch { changes, steps, .. } => Some((changes, steps)),
            _ => None,
        })
        .expect("A sends its first batch");
    assert!(steps.is_empty(), "steps wait for a peer that has them");
    b.apply_changes("dev-a", &changes, &[], 3_000).unwrap();
    assert_eq!(b.list_steps(&n).unwrap().len(), 0);

    // The same build, now announcing steps: everything withheld is sent.
    let mut sa2 = Session::new("dev-a", "dev-b");
    let _ = sa2.start(&mut a).unwrap();
    let replies = sa2
        .on_message(&mut a, hello_with_capabilities("dev-b", "B"), 4_000)
        .unwrap();
    let applied = drain_batches(&mut a, &mut sa2, &mut b, 4_000, replies);
    assert!(
        applied >= 3,
        "steps resent after the upgrade, got {applied}"
    );
    assert_eq!(
        steps_text(&b, &n),
        vec![("step 1".into(), false), ("step 2".into(), false)]
    );
    // The peer's marker is cleared once every batch was acknowledged.
    assert_eq!(
        a.get_peer("dev-b").unwrap().unwrap().steps_resend_from,
        None
    );
}

// ---- per-step merge rules -----------------------------------------------------

#[test]
fn ticking_and_renaming_one_step_on_two_devices_both_survive() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 2);
    sync_both_ways(&mut a, &mut b, 2_000);

    let id = a.list_steps(&n).unwrap()[0].id.clone();
    a.set_step_text_at(&id, "renamed on A", 3_000).unwrap();
    b.set_step_done_at(&id, true, 3_100).unwrap();

    sync_both_ways(&mut a, &mut b, 4_000);
    assert_eq!(
        steps_text(&a, &n),
        vec![("renamed on A".into(), true), ("step 1".into(), false)]
    );
    assert_eq!(steps_text(&b, &n), steps_text(&a, &n));
}

#[test]
fn editing_a_deleted_step_does_not_resurrect_it() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 1);
    sync_both_ways(&mut a, &mut b, 2_000);

    let id = a.list_steps(&n).unwrap()[0].id.clone();
    // A deletes; B, not knowing yet, renames the step with a later clock.
    a.remove_step_at(&id, 3_000).unwrap();
    b.set_step_text_at(&id, "renamed on B", 3_100).unwrap();

    // The deletion reaches B, then B's rename reaches A.
    sync_both_ways(&mut a, &mut b, 4_000);
    sync_both_ways(&mut a, &mut b, 4_100);

    // The step is gone everywhere: the rename did not resurrect it.
    assert!(a.list_steps(&n).unwrap().is_empty());
    assert!(b.list_steps(&n).unwrap().is_empty());
    // The tombstone keeps the newer text, so a later undo shows it.
    let text: String = a
        .conn()
        .query_row("SELECT text FROM note_steps WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(text, "renamed on B");
}

#[test]
fn undoing_a_synchronized_deletion_restores_the_step() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 1);
    sync_both_ways(&mut a, &mut b, 2_000);

    let id = a.list_steps(&n).unwrap()[0].id.clone();
    a.remove_step_at(&id, 3_000).unwrap();
    sync_both_ways(&mut a, &mut b, 3_500);
    assert!(b.list_steps(&n).unwrap().is_empty());

    // The undo: a newer explicit undelete clears the tombstone everywhere.
    a.restore_step_at(&id, 4_000).unwrap();
    sync_both_ways(&mut a, &mut b, 4_500);
    assert_eq!(steps_text(&b, &n), vec![("step 0".into(), false)]);
}

#[test]
fn concurrent_insertions_preserve_every_step_and_converge_to_one_order() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 1);
    sync_both_ways(&mut a, &mut b, 2_000);

    // Both devices insert between the same neighbours while apart.
    a.add_steps_at(
        &n,
        Some(&a.list_steps(&n).unwrap()[0].id),
        &[crate::NewStep {
            text: "from A".into(),
            done: false,
        }],
        3_000,
    )
    .unwrap();
    b.add_steps_at(
        &n,
        Some(&b.list_steps(&n).unwrap()[0].id),
        &[crate::NewStep {
            text: "from B".into(),
            done: false,
        }],
        3_100,
    )
    .unwrap();

    sync_both_ways(&mut a, &mut b, 4_000);
    sync_both_ways(&mut a, &mut b, 4_100);
    let ta = steps_text(&a, &n);
    let tb = steps_text(&b, &n);
    assert_eq!(ta.len(), 3, "no step was dropped");
    assert_eq!(ta, tb, "the same data shows the same order");
    // The step each device created is still on its own side after the merge.
    assert!(ta.iter().any(|(t, _)| t == "from A"));
    assert!(ta.iter().any(|(t, _)| t == "from B"));
}

#[test]
fn concurrent_step_moves_converge_deterministically() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 3);
    sync_both_ways(&mut a, &mut b, 2_000);

    let steps_a = a.list_steps(&n).unwrap();
    let steps_b = b.list_steps(&n).unwrap();
    // Both devices move the same step while apart; B's move is later, so
    // B's rank wins and both devices show the same order.
    a.move_step_at(&steps_a[0].id, Some(&steps_a[2].id), 3_000)
        .unwrap();
    b.move_step_at(&steps_b[0].id, None, 3_100).unwrap();

    sync_both_ways(&mut a, &mut b, 4_000);
    assert_eq!(steps_text(&a, &n), steps_text(&b, &n));
    let order: Vec<String> = a
        .list_steps(&n)
        .unwrap()
        .into_iter()
        .map(|s| s.text)
        .collect();
    assert_eq!(order, vec!["step 0", "step 1", "step 2"]);
}

#[test]
fn replaying_a_sync_batch_is_idempotent() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 2);
    a.remove_step(&a.list_steps(&n).unwrap()[0].id).unwrap();
    a.add_steps_at(
        &n,
        None,
        &[crate::NewStep {
            text: "new".into(),
            done: true,
        }],
        3_000,
    )
    .unwrap();
    a.set_step_done_at(&a.list_steps(&n).unwrap()[0].id, true, 3_100)
        .unwrap();

    let export = a.changes_since(0, 1_000, true).unwrap();
    let first = b
        .apply_changes("dev-a", &export.changes, &export.steps, 4_000)
        .unwrap();
    assert!(first.applied > 0);
    let second = b
        .apply_changes("dev-a", &export.changes, &export.steps, 4_100)
        .unwrap();
    assert_eq!(second.applied, 0, "replaying changes nothing");
    assert_eq!(steps_text(&b, &n), steps_text(&a, &n));
}

#[test]
fn purging_a_note_removes_its_steps_on_every_device() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 2);
    sync_both_ways(&mut a, &mut b, 2_000);

    a.trash_note_at(&n, 3_000).unwrap();
    a.purge_note(&n).unwrap();
    sync_both_ways(&mut a, &mut b, 4_000);
    sync_both_ways(&mut a, &mut b, 4_100);

    assert!(a.get_note(&n).is_err());
    assert!(b.get_note(&n).is_err());
    for db in [&a, &b] {
        let left: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM note_steps WHERE note_id = ?1",
                [&n],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 0, "purged steps are gone for good");
    }
}

#[test]
fn a_step_for_an_unknown_note_is_held_and_applied_exactly_once_when_the_note_arrives() {
    let (mut a, mut b) = setup_pair();
    // A creates a note and its steps, but only the steps reach B first:
    // export everything, apply only the step changes.
    let n = note_with_steps(&mut a, 2);
    a.set_step_done_at(&a.list_steps(&n).unwrap()[0].id, true, 1_500)
        .unwrap();

    let export = a.changes_since(0, 1_000, true).unwrap();
    b.apply_changes("dev-a", &[], &export.steps, 2_000).unwrap();
    assert_eq!(b.list_steps(&n).unwrap().len(), 0, "held, not applied");
    let held: i64 = b
        .conn()
        .query_row("SELECT COUNT(*) FROM sync_held_steps", [], |r| r.get(0))
        .unwrap();
    assert!(held >= 3, "step changes wait in the holding table");

    // Now the note itself arrives; the held changes apply once.
    b.apply_changes("dev-a", &export.changes, &[], 2_100)
        .unwrap();
    assert_eq!(
        steps_text(&b, &n),
        vec![("step 0".into(), true), ("step 1".into(), false)]
    );
    let held_after: i64 = b
        .conn()
        .query_row("SELECT COUNT(*) FROM sync_held_steps", [], |r| r.get(0))
        .unwrap();
    assert_eq!(held_after, 0, "holding is emptied");

    // Replaying the same step changes changes nothing (no duplicates).
    b.apply_changes("dev-a", &[], &export.steps, 2_200).unwrap();
    assert_eq!(
        steps_text(&b, &n),
        vec![("step 0".into(), true), ("step 1".into(), false)]
    );
}

#[test]
fn merging_steps_in_a_chain_of_three_devices_converges() {
    let mut a = Db::open_in_memory().unwrap();
    let mut b = Db::open_in_memory().unwrap();
    let mut c = Db::open_in_memory().unwrap();
    a.set_sync_device_id("dev-a").unwrap();
    b.set_sync_device_id("dev-b").unwrap();
    c.set_sync_device_id("dev-c").unwrap();
    a.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
    b.upsert_peer("dev-a", "A", &[], 1_000).unwrap();
    b.upsert_peer("dev-c", "C", &[], 1_000).unwrap();
    c.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
    // A and C never sync directly; everything flows through B.

    let n = note_with_steps(&mut a, 2);
    sync_one_way(&mut a, "dev-a", &mut b, "dev-b", 2_000);
    let id = b.list_steps(&n).unwrap()[0].id.clone();
    b.set_step_text_at(&id, "edited on B", 2_100).unwrap();
    sync_one_way(&mut b, "dev-b", &mut c, "dev-c", 2_200);
    c.set_step_done_at(&id, true, 2_300).unwrap();
    // B forwards C's change back to A.
    sync_one_way(&mut c, "dev-c", &mut b, "dev-b", 2_400);
    sync_one_way(&mut b, "dev-b", &mut a, "dev-a", 2_500);

    let ta = steps_text(&a, &n);
    let tc = steps_text(&c, &n);
    assert_eq!(
        ta,
        vec![("edited on B".into(), true), ("step 1".into(), false)]
    );
    assert_eq!(ta, tc);
}

#[test]
fn two_devices_each_adding_80_steps_keep_all_160_and_block_further_adds() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 1);
    sync_both_ways(&mut a, &mut b, 2_000);

    let lines = |prefix: &str| {
        (0..80)
            .map(|i| crate::NewStep {
                text: format!("{prefix} {i}"),
                done: false,
            })
            .collect::<Vec<_>>()
    };
    a.add_steps_at(&n, None, &lines("A"), 3_000).unwrap();
    b.add_steps_at(&n, None, &lines("B"), 3_100).unwrap();
    sync_both_ways(&mut a, &mut b, 4_000);
    sync_both_ways(&mut a, &mut b, 4_100);

    assert_eq!(a.list_steps(&n).unwrap().len(), 161);
    assert_eq!(steps_text(&a, &n), steps_text(&b, &n));

    // 160 steps is over the local limit: adding is blocked on both sides
    // until the note is back under it.
    assert_eq!(
        a.add_steps_at(
            &n,
            None,
            &[crate::NewStep {
                text: "one more".into(),
                done: false
            }],
            5_000
        )
        .unwrap_err()
        .code(),
        "step_limit"
    );
    assert_eq!(
        b.add_steps_at(
            &n,
            None,
            &[crate::NewStep {
                text: "one more".into(),
                done: false
            }],
            5_100
        )
        .unwrap_err()
        .code(),
        "step_limit"
    );
}

#[test]
fn an_interrupted_session_resumes_without_duplicate_steps() {
    let (mut a, mut b) = setup_pair();
    let n = note_with_steps(&mut a, 3);

    // The session drops after B received the batch but before A got the Ack.
    let mut sa1 = Session::new("dev-a", "dev-b");
    let mut sb1 = Session::new("dev-b", "dev-a");
    let mut q_a_to_b: VecDeque<Message> = sa1.start(&mut a).unwrap().into();
    let mut q_b_to_a: VecDeque<Message> = sb1.start(&mut b).unwrap().into();
    let mut batches = 0;
    'outer: while !q_a_to_b.is_empty() || !q_b_to_a.is_empty() {
        if let Some(msg) = q_a_to_b.pop_front() {
            let is_batch = matches!(msg, Message::Batch { .. });
            let reply = sb1.on_message(&mut b, msg, 2_000).unwrap();
            q_b_to_a.extend(reply);
            // The connection drops after B applied the batch, before its Ack
            // reaches A.
            if is_batch {
                break 'outer;
            }
        }
        if let Some(msg) = q_b_to_a.pop_front() {
            let reply = sa1.on_message(&mut a, msg, 2_000).unwrap();
            q_a_to_b.extend(reply);
        }
    }
    assert!(!sa1.is_finished(), "the session was cut before the ack");
    assert_eq!(b.list_steps(&n).unwrap().len(), 3, "the batch was applied");

    // A fresh session resumes; nothing is duplicated.
    let mut sa2 = Session::new("dev-a", "dev-b");
    let mut sb2 = Session::new("dev-b", "dev-a");
    drive(&mut a, &mut b, &mut sa2, &mut sb2, 3_000);
    assert_eq!(b.list_steps(&n).unwrap().len(), 3);
}

/// Runs both sessions to completion.
fn drive(a: &mut Db, b: &mut Db, sa: &mut Session, sb: &mut Session, now_ms: i64) {
    let mut q_a_to_b: VecDeque<Message> = sa.start(a).unwrap().into();
    let mut q_b_to_a: VecDeque<Message> = sb.start(b).unwrap().into();
    while !q_a_to_b.is_empty() || !q_b_to_a.is_empty() {
        if let Some(msg) = q_a_to_b.pop_front() {
            let reply = sb.on_message(b, msg, now_ms).unwrap();
            q_b_to_a.extend(reply);
        }
        if let Some(msg) = q_b_to_a.pop_front() {
            let reply = sa.on_message(a, msg, now_ms).unwrap();
            q_a_to_b.extend(reply);
        }
    }
}
