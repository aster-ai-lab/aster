---
name: presenton
description: Answer one Presenton view step when parameters.step is outline, structure, or slide. Return one JSON object for that step. Do not use this skill to create a file or start a deck; the host view calls the step.
version: 1.0.0
---

# 演示文稿

The `aster/presenton` workbench builds a slide deck from a topic in three kinds of turns. Each turn is one step. The view sends the step in `parameters.step`; the agent answers with one JSON object.

## Steps

- `outline`: from `content`, `n_slides` (1–20), `language`, `tone`, `verbosity`, `include_title_slide`, and `instructions`, write one Markdown outline per slide. Each outline has a `##` title and only audience-facing content. Output: `{"step":"outline","outline":["## …", …]}`.
- `structure`: from `template` and the `outline` list, pick one layout of the template for each slide. Output: `{"step":"structure","layouts":["<layout id>", …],"source":"model"|"rule"}`. When the model cannot answer, layouts are assigned in template order and `source` is `rule`.
- `slide`: from `template`, `layout`, the slide's `outline`, and `slide_number`, fill the layout's content schema. Text respects each field's character limits. Icon fields name one icon from the package's icon list. Image fields carry an English `image_prompt`. Output: `{"step":"slide","layout":"<id>","content":{…, "__speaker_note__":"…"}}`.

A step that cannot run returns `{"error":"<reason>"}`.

## Rules

1. Write slide content, not instructions about slides. Do not describe charts, layouts, colors, or positions in any text field.
2. Use the language in `language` for every audience-facing field. Icon names and image prompts stay English.
3. Do not invent sources, URLs, or citations.
4. Do not copy API keys, tokens, or authorization headers into a reply. Model calls go through the aster gateway; the member does not paste a supplier key.
