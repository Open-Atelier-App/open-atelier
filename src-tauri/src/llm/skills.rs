use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct Skill {
    pub name: String,
    pub instructions: String,
}

#[derive(Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub preview: String,
}

const BUNDLED_LLM_FUNCTIONS_SKILL: &str =
    include_str!("../../resources/skills/llm-functions-v1.md");

struct DefaultSkill {
    name: &'static str,
    content: &'static str,
}

/// Ready-made skills bundled with the app, shown in Settings > Skills as
/// suggestions the user can add to their active profile with one click —
/// unlike BUNDLED_LLM_FUNCTIONS_SKILL above, these are NOT auto-injected;
/// they only take effect once copied into the profile's `skills/` folder,
/// same as any skill the user writes by hand.
const DEFAULT_SKILLS: &[DefaultSkill] = &[
    DefaultSkill {
        name: "meeting-notes",
        content: include_str!("../../resources/default-skills/meeting-notes.md"),
    },
    DefaultSkill {
        name: "presentation-builder",
        content: include_str!("../../resources/default-skills/presentation-builder.md"),
    },
    DefaultSkill {
        name: "code-reviewer",
        content: include_str!("../../resources/default-skills/code-reviewer.md"),
    },
    DefaultSkill {
        name: "research-report",
        content: include_str!("../../resources/default-skills/research-report.md"),
    },
    DefaultSkill {
        name: "spreadsheet-analyst",
        content: include_str!("../../resources/default-skills/spreadsheet-analyst.md"),
    },
    DefaultSkill {
        name: "read-aloud-narrator",
        content: include_str!("../../resources/default-skills/read-aloud-narrator.md"),
    },
    DefaultSkill {
        name: "email-drafting",
        content: include_str!("../../resources/default-skills/email-drafting.md"),
    },
    DefaultSkill {
        name: "resume-cv-writer",
        content: include_str!("../../resources/default-skills/resume-cv-writer.md"),
    },
    DefaultSkill {
        name: "data-analyst",
        content: include_str!("../../resources/default-skills/data-analyst.md"),
    },
    DefaultSkill {
        name: "translator",
        content: include_str!("../../resources/default-skills/translator.md"),
    },
    DefaultSkill {
        name: "brainstorm-facilitator",
        content: include_str!("../../resources/default-skills/brainstorm-facilitator.md"),
    },
    DefaultSkill {
        name: "github-code-reviewer",
        content: include_str!("../../resources/default-skills/github-code-reviewer.md"),
    },
];

fn skills_dir(profile_root: &str) -> PathBuf {
    Path::new(profile_root).join("skills")
}

/// Reads a workspace's own `context.md` index (see the "Project context
/// file" section of the LLM Functions protocol), if it has one — used to
/// share a project's context with its parent/child sub-projects. Returns
/// `None` for a missing or empty file rather than an error: not every
/// workspace has gotten a context.md written yet, and that's routine, not
/// exceptional.
pub fn read_context_md(workspace_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(Path::new(workspace_path).join("context.md")).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn load_skills(profile_root: &str) -> Vec<Skill> {
    let dir = skills_dir(profile_root);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(instructions) = std::fs::read_to_string(&path) {
            let trimmed = instructions.trim();
            if !trimmed.is_empty() {
                skills.push(Skill {
                    name: name.to_string(),
                    instructions: trimmed.to_string(),
                });
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

#[tauri::command]
pub fn skill_list(profile_root: String) -> Vec<SkillInfo> {
    load_skills(&profile_root)
        .into_iter()
        .map(|s| {
            let preview = s
                .instructions
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect();
            SkillInfo {
                name: s.name,
                preview,
            }
        })
        .collect()
}

/// Lists bundled default skills, regardless of whether they're already
/// installed in the given profile — the frontend cross-references against
/// `skill_list` to decide which ones to show as "Add" vs. already present.
#[tauri::command]
pub fn default_skill_list() -> Vec<SkillInfo> {
    DEFAULT_SKILLS
        .iter()
        .map(|s| {
            let preview = s
                .content
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect();
            SkillInfo {
                name: s.name.to_string(),
                preview,
            }
        })
        .collect()
}

/// Copies a bundled default skill's content into the profile's `skills/`
/// folder so it starts applying immediately, through the exact same
/// `load_skills` path as a skill the user wrote by hand. Overwrites if the
/// user already has a file with that name.
#[tauri::command]
pub fn default_skill_install(
    profile_root: String,
    name: String,
) -> std::result::Result<(), String> {
    let skill = DEFAULT_SKILLS
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("Unknown default skill: {name}"))?;

    let dir = skills_dir(&profile_root);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create skills directory: {e}"))?;

    let path = dir.join(format!("{}.md", skill.name));
    std::fs::write(&path, skill.content).map_err(|e| format!("Cannot write skill file: {e}"))?;

    Ok(())
}

pub fn build_context(
    profile_root: &str,
    workspace_name: &str,
    workspace_path: &str,
    provider: &str,
    is_first_message: bool,
) -> String {
    let mut block = if is_first_message {
        format!(
            "You are the AI assistant inside Open Atelier, a local-first creative workspace \
             application. Your role is to help the user work with their project files, answer \
             questions about their content, and assist with writing and editing.\n\
             \n\
             ## Current workspace\n\
             - Name: \"{workspace_name}\"\n\
             - Path: {workspace_path}\n\
             \n\
             ## How to interact with files\n\
             You interact with the user's files EXCLUSIVELY through the trigger protocol \
             described below. You do NOT have traditional tool/function calling. Instead, you \
             emit structured triggers in your response text using the >>>[ACTION]<<< format.\n\
             \n\
             ## Critical rules\n\
             - **Don't dump long-form document or code content directly in the chat message.** \
               When the user wants a document, script, or anything they'll keep working with, \
               use a trigger (CREATE, WRITE, APPEND) to write it to a file, then send a short \
               MESSAGE confirming what you did. This includes NOT previewing or narrating the \
               content beforehand (\"here's what I'll write...\") — go straight to the triggers, \
               the content only ever appears inside the WRITE trigger itself, never in your \
               chat text before or after it.\n\
             - When the user asks you to write, create, or draft something substantial (a \
               document, a real code file, notes, an essay, a plan, etc.), create a file using \
               triggers rather than pasting it into chat.\n\
             - Short, illustrative answers are fine inline: a one-line command, a quick fact, \
               or a few-line snippet used to explain something in conversation. Use judgment — \
               the goal is avoiding long content dumped into chat, not banning small examples.\n\
             - When the user asks about existing file content, use READ to fetch it, then \
               summarize in chat. Do not reproduce entire files in chat messages.\n\
             - The user can see files in the sidebar and file viewer. Use that.\n\
             - Keep `context.md` at the project root up to date (path + one-line summary per \
               file) whenever you create, write, rename, or delete a file — see the full \
               protocol below for how. It's your own working index, not user-facing content.\n\
             \n\
             ## Supported file types\n\
             You can read and write text-based files: HTML, Markdown (.md), source code, CSS, \
             JSON, YAML, TOML, plain text, and similar formats. You can also CREATE real Word, \
             Excel, and PowerPoint documents with CREATE_DOCX/CREATE_XLSX/CREATE_PPTX, and real \
             MP3 audio narration with CREATE_MP3 (see the trigger protocol below for their \
             content format) — but you cannot READ an existing DOCX/XLSX/PPTX/PDF/MP3's content; \
             for those, direct the user to open it themselves (the file viewer offers an \"open \
             in default app\" button for binary files)."
        )
    } else {
        format!(
            "You are assisting inside Open Atelier. Current workspace: \"{workspace_name}\" \
             at {workspace_path}. Interact with files ONLY through >>>[ACTION]<<< triggers. \
             Don't dump long-form document or code content in chat — write it to a file using \
             CREATE/WRITE triggers, then confirm briefly via MESSAGE. Never preview or narrate \
             the content first; go straight to the triggers. Short inline snippets and quick \
             answers are fine. Summarize rather than reproducing file contents in full. Keep \
             context.md at the project root up to date whenever you touch a file."
        )
    };

    let skills = load_skills(profile_root);
    if !skills.is_empty() {
        block.push_str("\n\n## Atelier Skills\nThe following skills are loaded for this profile:");
        for skill in &skills {
            block.push_str(&format!("\n\n### {}\n{}", skill.name, skill.instructions));
        }
    }

    // Always inject the LLM Functions protocol (embedded at compile time)
    let has_llm_functions = skills.iter().any(|s| s.name.starts_with("llm-functions"));
    if !has_llm_functions {
        block.push_str("\n\n");
        block.push_str(BUNDLED_LLM_FUNCTIONS_SKILL.trim());
    }

    // Append runtime permission block
    let perm_level =
        super::permissions::get_level_for_provider(provider).unwrap_or_else(|_| "chat_only".into());
    let perm_label =
        super::permissions::level_label(&perm_level).unwrap_or_else(|_| "Chat Only".into());
    let allowed = super::permissions::allowed_triggers_for_level(&perm_level)
        .unwrap_or_else(|_| vec!["MESSAGE".into()]);
    let perm_block =
        crate::triggers::formatter::format_runtime_permission_block(&perm_label, &allowed);
    block.push_str("\n\n");
    block.push_str(&perm_block);

    block
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_profile() -> String {
        let dir =
            std::env::temp_dir().join(format!("atelier_skills_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_str().unwrap().to_string()
    }

    #[test]
    fn default_skills_are_all_non_empty_and_uniquely_named() {
        let list = default_skill_list();
        assert_eq!(list.len(), DEFAULT_SKILLS.len());
        let mut names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            list.len(),
            "default skill names must be unique"
        );
        for s in &list {
            assert!(!s.preview.is_empty(), "{} has an empty preview", s.name);
        }
    }

    #[test]
    fn default_skill_install_writes_a_file_load_skills_picks_up() {
        let profile_root = temp_profile();
        default_skill_install(profile_root.clone(), "meeting-notes".to_string()).unwrap();

        let skills = load_skills(&profile_root);
        assert!(skills.iter().any(|s| s.name == "meeting-notes"));

        std::fs::remove_dir_all(&profile_root).ok();
    }

    #[test]
    fn default_skill_install_rejects_unknown_name() {
        let profile_root = temp_profile();
        let err = default_skill_install(profile_root.clone(), "not-a-real-skill".to_string())
            .unwrap_err();
        assert!(err.contains("Unknown default skill"));

        std::fs::remove_dir_all(&profile_root).ok();
    }
}
