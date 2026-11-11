use super::{Context, Saved, Snapshot};
use crate::prompt_images::{self, PastedImage};
use anyhow::Context as _;
use aster_shell::session::{info::Info, persistence};
use std::{
    fs,
    io::{ErrorKind, Read},
    path::Path,
};

const FILE: &str = "unsent-input.json";
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn read(directory: &Path) -> anyhow::Result<Option<Saved>> {
    let path = directory.join(FILE);
    let file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "Unsent input exceeds the snapshot limit"
    );
    let saved: Saved = serde_json::from_slice(&bytes).map_err(|error| {
        anyhow::anyhow!(
            "Invalid unsent input at line {} column {}",
            error.line(),
            error.column()
        )
    })?;
    anyhow::ensure!(saved.schema == 1, "Unsupported unsent input schema");
    Ok(Some(saved))
}

pub(super) fn load(info: &Info) -> anyhow::Result<Option<Saved>> {
    load_at(&persistence::session_dir(info))
}

pub(super) fn load_at(directory: &Path) -> anyhow::Result<Option<Saved>> {
    let Some(mut saved) = read(directory)? else {
        return Ok(None);
    };
    let images = directory.join("images");
    for context in &mut saved.contexts {
        let mut prepare = |image: &mut PastedImage| -> anyhow::Result<()> {
            let path = image
                .session_image_path
                .as_ref()
                .context("Image path is absent")?;
            anyhow::ensure!(
                dunce::canonicalize(path)?.starts_with(dunce::canonicalize(&images)?),
                "Recovered image is outside its session"
            );
            let (bytes, _) =
                prompt_images::load_for_send(image).context("Recovered image is unavailable")?;
            image.encoded_bytes = Some(bytes.into());
            image.prepare_preview_blocking();
            image.encoded_bytes = None;
            Ok(())
        };
        visit_images(context, &mut prepare)?;
        validate(context)?;
    }
    Ok(Some(saved))
}

pub(super) fn save(snapshot: Snapshot) -> anyhow::Result<()> {
    let directory = persistence::ensure_owner_only_session_dir(&snapshot.info)?;
    save_at(snapshot, &directory)
}

pub(super) fn save_at(mut snapshot: Snapshot, directory: &Path) -> anyhow::Result<()> {
    if snapshot.merge_disk
        && let Some(mut previous) = read(directory)?
    {
        previous.contexts.append(&mut snapshot.saved.contexts);
        snapshot.saved = previous;
    }
    snapshot
        .saved
        .contexts
        .retain(|context| !context.queue.is_empty() || context.has_draft());
    let path = directory.join(FILE);
    if snapshot.saved.contexts.is_empty() {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        };
    }
    let mut persist = |image: &mut PastedImage| -> anyhow::Result<()> {
        if image.session_image_path.is_none() {
            prompt_images::persist_to_session(image, &directory.join("images"))?;
        }
        Ok(())
    };
    for context in &mut snapshot.saved.contexts {
        visit_images(context, &mut persist)?;
    }
    let json = serde_json::to_string(&snapshot.saved)?;
    anyhow::ensure!(
        json.len() as u64 <= MAX_BYTES,
        "Unsent input exceeds the snapshot limit"
    );
    aster_config::fs_atomic::write_atomically(&path, &json, Some(0o600))
        .context("Could not write unsent input")
}

fn visit_images(
    context: &mut Context,
    visit: &mut impl FnMut(&mut PastedImage) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for row in &mut context.queue {
        for image in &mut row.images {
            visit(image)?;
        }
    }
    context.for_each_prompt(|prompt| {
        for image in &mut prompt.images {
            visit(image)?;
        }
        Ok(())
    })
}

fn validate(context: &mut Context) -> anyhow::Result<()> {
    context.for_each_prompt(|prompt| {
        anyhow::ensure!(
            prompt.text.is_char_boundary(prompt.cursor),
            "Invalid saved cursor"
        );
        validate_chips(&prompt.text, &prompt.chip_elements)
    })?;
    for row in &context.queue {
        validate_chips(&row.text, &row.chip_elements)?;
    }
    Ok(())
}
fn validate_chips(text: &str, chips: &[crate::app::agent::ChipElement]) -> anyhow::Result<()> {
    for chip in chips {
        anyhow::ensure!(
            chip.range.start <= chip.range.end && text.get(chip.range.clone()).is_some(),
            "Invalid saved chip range"
        );
    }
    Ok(())
}
