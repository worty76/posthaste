use std::collections::VecDeque;

use crate::sync::{
    check_frame_len, decode_frame, encode_frame, Message, Session, MAX_FRAME_BYTES,
    PROTOCOL_VERSION,
};
use crate::{Db, NewNote, NoteQuery};

fn note(content: &str) -> NewNote {
    NewNote {
        content: content.into(),
        category: None,
        source: None,
    }
}

fn setup_paired() -> (Db, Db) {
    let mut a = Db::open_in_memory().unwrap();
    let mut b = Db::open_in_memory().unwrap();
    a.set_sync_device_id("dev-a").unwrap();
    b.set_sync_device_id("dev-b").unwrap();
    a.upsert_peer("dev-b", "B", &[], 1_000).unwrap();
    b.upsert_peer("dev-a", "A", &[], 1_000).unwrap();
    (a, b)
}

fn drive_sessions(
    a: &mut Db,
    b: &mut Db,
    sa: &mut Session,
    sb: &mut Session,
    now_ms: i64,
    drop_after_batches: Option<usize>,
) -> usize {
    let mut q_a_to_b: VecDeque<Message> = sa.start(a).unwrap().into();
    let mut q_b_to_a: VecDeque<Message> = sb.start(b).unwrap().into();
    let mut batches_seen = 0usize;

    while !q_a_to_b.is_empty() || !q_b_to_a.is_empty() {
        if let Some(msg) = q_a_to_b.pop_front() {
            let is_batch = matches!(msg, Message::Batch { .. });
            // Round-trip through framing to verify encode/decode on every message.
            let frame = encode_frame(&msg).unwrap();
            let decoded = decode_frame(&frame).unwrap();
            let reply = sb.on_message(b, decoded, now_ms).unwrap();
            if is_batch {
                batches_seen += 1;
                if drop_after_batches.is_some_and(|limit| batches_seen >= limit) {
                    break;
                }
            }
            q_b_to_a.extend(reply);
        }
        if let Some(msg) = q_b_to_a.pop_front() {
            let frame = encode_frame(&msg).unwrap();
            let decoded = decode_frame(&frame).unwrap();
            let reply = sa.on_message(a, decoded, now_ms).unwrap();
            q_a_to_b.extend(reply);
        }
    }
    batches_seen
}

#[test]
fn a_full_session_syncs_both_ways() {
    let (mut a, mut b) = setup_paired();
    for i in 0..1_200 {
        a.create_note_at(note(&format!("note-{i}")), 10_000 + i)
            .unwrap();
    }
    let from_b = b.create_note_at(note("from B"), 20_000).unwrap();

    let mut sa = Session::new("dev-a", "dev-b").with_name("A");
    let mut sb = Session::new("dev-b", "dev-a").with_name("B");
    let batches = drive_sessions(&mut a, &mut b, &mut sa, &mut sb, 30_000, None);

    // 1,200 notes x 2 fields (created + content) = 2,400 changes -> 5 batches of 500.
    assert!(batches >= 3, "expected multiple batches, got {batches}");
    assert!(sa.is_finished());
    assert!(sb.is_finished());
    assert_eq!(sa.refused_reason(), None);
    assert_eq!(sb.refused_reason(), None);

    assert_eq!(a.info().unwrap().note_count, 1_201);
    assert_eq!(b.info().unwrap().note_count, 1_201);
    assert_eq!(a.get_note(&from_b.id).unwrap().content, "from B");
}

#[test]
fn a_session_cut_halfway_finishes_next_time() {
    let (mut a, mut b) = setup_paired();
    for i in 0..700 {
        a.create_note_at(note(&format!("n-{i}")), 1_000 + i)
            .unwrap();
    }

    // First session drops right after B receives the second Batch (before A gets its Ack).
    let mut sa1 = Session::new("dev-a", "dev-b");
    let mut sb1 = Session::new("dev-b", "dev-a");
    let batches1 = drive_sessions(&mut a, &mut b, &mut sa1, &mut sb1, 5_000, Some(2));
    assert_eq!(batches1, 2);
    assert!(!sa1.is_finished());

    // Second session resumes and finishes cleanly with no duplicates.
    let mut sa2 = Session::new("dev-a", "dev-b");
    let mut sb2 = Session::new("dev-b", "dev-a");
    drive_sessions(&mut a, &mut b, &mut sa2, &mut sb2, 6_000, None);
    assert!(sa2.is_finished());
    assert!(sb2.is_finished());

    let all_b = b
        .list_notes(&NoteQuery {
            limit: Some(1000),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all_b.len(), 700);
}

#[test]
fn an_unpaired_device_is_refused_before_any_notes_are_read() {
    let mut host = Db::open_in_memory().unwrap();
    host.set_sync_device_id("host").unwrap();
    host.create_note_at(note("private note"), 1_000).unwrap();

    let mut session = Session::new("host", "stranger");
    let reply = session
        .on_message(
            &mut host,
            Message::Hello {
                protocol: PROTOCOL_VERSION,
                device_id: "stranger".into(),
                name: "Stranger".into(),
                pair_secret: None,
            },
            2_000,
        )
        .unwrap();

    assert_eq!(reply.len(), 1);
    assert!(matches!(reply[0], Message::Refused { .. }));
    assert!(session.is_finished());
    assert!(session.refused_reason().is_some());
    assert!(!host.is_paired("stranger").unwrap());
}

#[test]
fn frames_over_the_size_limit_are_refused() {
    let too_big = (MAX_FRAME_BYTES as u32 + 1).to_be_bytes();
    assert!(decode_frame(&too_big).is_err());
    assert!(check_frame_len(MAX_FRAME_BYTES + 1).is_err());
    assert!(check_frame_len(MAX_FRAME_BYTES).is_ok());
}
