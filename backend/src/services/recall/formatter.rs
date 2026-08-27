use super::auto_recall::RecalledMemory;
use super::strategy::RecallSource;

pub struct RecallFormatter;

impl RecallFormatter {
    pub fn format_for_prompt(memories: &[RecalledMemory], max_tokens: usize) -> String {
        if memories.is_empty() {
            return String::new();
        }

        let mut output = String::from("<memory-context>\n");
        let mut current_len = output.len();
        let approx_max_chars = max_tokens * 4;

        for mem in memories {
            let prefix = match mem.source {
                RecallSource::L1Atom => format!("[{}] ", mem.atom_type_str()),
                RecallSource::L2Scene => "[scene] ".to_string(),
                RecallSource::L3Persona => "[persona] ".to_string(),
                RecallSource::BeliefEdge => "[belief] ".to_string(),
            };
            let line = format!("- {}{}\n", prefix, mem.content);

            if current_len + line.len() > approx_max_chars {
                output.push_str("…(truncated)\n");
                break;
            }

            output.push_str(&line);
            current_len += line.len();
        }

        output.push_str("</memory-context>");
        output
    }

    pub fn format_persona_context(persona_content: &str) -> String {
        format!("<persona>\n{}\n</persona>", persona_content)
    }

    pub fn format_scene_navigation(scenes: &[(String, String)]) -> String {
        if scenes.is_empty() {
            return String::new();
        }

        let mut output = String::from("<scene-navigation>\n");
        for (name, summary) in scenes {
            output.push_str(&format!("- {}: {}\n", name, summary));
        }
        output.push_str("</scene-navigation>");
        output
    }
}
