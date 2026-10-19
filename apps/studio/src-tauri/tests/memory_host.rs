//! The studio lib test exe exits before main on Windows. This process is the
//! one that calls the shipped host functions.

use std::io::ErrorKind;
use std::net::TcpListener;

use a3s_studio_lib::{
    is_toboss_room_session, memory_directives, merge_memory_turn, merge_room_memory_turn,
    private_memory, room_memory_directives, room_retain, room_turn_ask, toboss_room_parts,
};
use serde_json::json;

fn main() {
    if let Err(message) = prove() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn prove() -> Result<(), String> {
    let ask_prompt = "Group: launch\nAda: hello\n\nAda asked: ship Friday";
    let ask = room_turn_ask(ask_prompt).ok_or("group host line did not yield an ask")?;
    if ask != "ship Friday" {
        return Err(format!("group host line yielded {ask}"));
    }
    let note = "群决定周五发布。";
    let room_lines = room_memory_directives(Some(note));
    let personal_lines = memory_directives(Some(note));
    let room_rule = room_lines
        .first()
        .ok_or("room directives were empty")?
        .clone();
    let personal_rule = personal_lines
        .first()
        .ok_or("personal directives were empty")?
        .clone();
    if room_rule != "只用下面列出的群事实。" {
        return Err(format!("room rule was {room_rule}"));
    }
    if room_lines.iter().any(|line| line == &personal_rule) {
        return Err("room directives included the personal memory rule".to_string());
    }
    let turn = json!({ "directives": ["This turn is a group."], "research": false });
    let merged =
        merge_room_memory_turn(Some(&turn), Some(note)).ok_or("room merge dropped the turn")?;
    let merged_lines = merged["directives"]
        .as_array()
        .ok_or("room merge has no directives")?;
    if merged_lines.first().and_then(|line| line.as_str()) != Some("This turn is a group.") {
        return Err("room merge dropped the host directive".to_string());
    }
    if !merged_lines
        .iter()
        .any(|line| line.as_str() == Some(room_rule.as_str()))
    {
        return Err("room merge omitted the room rule".to_string());
    }
    if merged_lines
        .iter()
        .any(|line| line.as_str() == Some(personal_rule.as_str()))
    {
        return Err("room merge included the personal memory rule".to_string());
    }

    let transcript = "Group: launch\nonly a transcript";
    if room_turn_ask(transcript).is_some() {
        return Err("transcript-only prompt yielded an ask".to_string());
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    listener
        .set_nonblocking(true)
        .map_err(|err| err.to_string())?;
    let origin = format!(
        "http://{}",
        listener.local_addr().map_err(|err| err.to_string())?
    );
    let member = "44444444-4444-4444-8444-444444444444";
    let room = "55555555-5555-4555-8555-555555555555";
    room_retain(
        &origin,
        "member-slug",
        "22222222-2222-4222-8222-222222222222",
        room,
        "33333333-3333-4333-8333-333333333333",
        transcript,
        "answer",
    );
    match listener.accept() {
        Err(err) if err.kind() == ErrorKind::WouldBlock => {}
        Ok(_) => return Err("transcript retain opened a socket".to_string()),
        Err(err) => return Err(err.to_string()),
    }

    let group_id = format!("toboss-{member}-room-{room}");
    let personal_id = format!("tobot-{member}");
    if toboss_room_parts(&group_id).is_none() {
        return Err("group session was not a room".to_string());
    }
    if private_memory(&group_id).is_some() {
        return Err("group session selected a personal bank".to_string());
    }
    if !is_toboss_room_session(&group_id) {
        return Err("group session skipped the room prompt".to_string());
    }
    if toboss_room_parts(&personal_id).is_some() {
        return Err("personal session was treated as a room".to_string());
    }
    if is_toboss_room_session(&personal_id) {
        return Err("personal session took the room prompt".to_string());
    }
    let personal = private_memory(&personal_id).ok_or("personal session had no personal bank")?;
    if personal.scope != "personal" {
        return Err(format!("personal scope was {}", personal.scope));
    }
    let personal_turn =
        merge_memory_turn(None, Some(note)).ok_or("personal merge dropped the note")?;
    let personal_merged = personal_turn["directives"]
        .as_array()
        .ok_or("personal merge has no directives")?;
    if !personal_merged
        .iter()
        .any(|line| line.as_str() == Some(personal_rule.as_str()))
    {
        return Err("personal merge omitted the personal rule".to_string());
    }
    if personal_merged
        .iter()
        .any(|line| line.as_str() == Some(room_rule.as_str()))
    {
        return Err("personal merge included the room rule".to_string());
    }

    println!("ask={ask}");
    println!("room_rule={room_rule}");
    println!("transcript_retain=stored_nothing");
    println!("personal_scope={}", personal.scope);
    println!("personal_path=off_room_bank");
    Ok(())
}
