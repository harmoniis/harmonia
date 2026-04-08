use harmonia_vault::{get_secret_for_component, init_from_env};

use crate::model::json_escape;

use super::clients::{
    clip_text, openrouter_api_key, request_openrouter, truth_seeking_prompt,
    preferred_truth_seeking_model,
};

const COMPONENT: &str = "parallel-agents-core";

const FALLBACK_VERIFY_TEMPLATE: &str = "You are the truth-seeking verification subagent. Use live web and X search when useful. Prioritize factual accuracy over style.\n\nOriginal user prompt:\n{PROMPT}\n\nCandidate answer:\n{RESPONSE}\n\nReply exactly in this format:\nVERIFY: yes|no|uncertain\nSOURCE: web|x|web+x|unknown\nNOTES: one concise sentence";

fn make_grok_verify_prompt(prompt: &str, response: &str) -> String {
    let template = harmonia_config_store::get_config(COMPONENT, "prompts", "grok-verification")
        .ok()
        .flatten()
        .unwrap_or_else(|| FALLBACK_VERIFY_TEMPLATE.to_string());
    // Template uses {PROMPT} and {RESPONSE} placeholders (or CL-style ~A)
    template
        .replace("{PROMPT}", prompt)
        .replace("{RESPONSE}", response)
        .replacen("~A", prompt, 1)
        .replacen("~A", response, 1)
}

pub(super) fn grok_verify_result(report: &str) -> (bool, String, String) {
    let lower = report.to_ascii_lowercase();
    let ok =
        lower.lines().any(|line| line.trim() == "verify: yes") || lower.contains("verify: yes");
    (ok, "grok-live".to_string(), clip_text(report.trim(), 240))
}

fn verify_with_grok(prompt: &str, response: &str) -> Result<(bool, String, String), String> {
    let key = openrouter_api_key()?.ok_or_else(|| {
        "missing secret: openrouter-api-key (vault component: openrouter-backend)".to_string()
    })?;
    let model = preferred_truth_seeking_model();
    let report = request_openrouter(&make_grok_verify_prompt(prompt, response), &model, &key)?;
    Ok(grok_verify_result(&report))
}

fn request_exa(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = harmonia_config_store::get_config(COMPONENT, "search-exa-tool", "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.exa.ai/search".to_string());
    let payload = format!("{{\"query\":\"{}\",\"numResults\":5}}", json_escape(query));
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("x-api-key: {api_key}"))
        .arg("-d")
        .arg(payload)
        .arg(endpoint)
        .output()
        .map_err(|e| format!("exa curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "exa curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn request_brave(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = harmonia_config_store::get_config(COMPONENT, "search-brave-tool", "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.search.brave.com/res/v1/web/search".to_string());
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-G")
        .arg("-H")
        .arg(format!("X-Subscription-Token: {api_key}"))
        .arg("--data-urlencode")
        .arg(format!("q={query}"))
        .arg(endpoint)
        .output()
        .map_err(|e| format!("brave curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "brave curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn make_verify_query(prompt: &str, response: &str) -> String {
    let p = prompt.chars().take(120).collect::<String>();
    let r = response.chars().take(180).collect::<String>();
    format!("verify this answer against the web: prompt={p} answer={r}")
}

pub(super) fn verify_with_search(prompt: &str, response: &str) -> (bool, String, String) {
    let _ = init_from_env();
    if response.trim().is_empty() {
        return (false, "none".to_string(), "empty-response".to_string());
    }
    if truth_seeking_prompt(prompt) {
        match verify_with_grok(prompt, response) {
            Ok(result) => return result,
            Err(e) => {
                if let Ok(Some(exa)) =
                    get_secret_for_component("parallel-agents-core", "exa_api_key")
                {
                    let q = make_verify_query(prompt, response);
                    match request_exa(&q, &exa) {
                        Ok(payload) => {
                            let ok = payload.contains("\"results\"");
                            return (
                                ok,
                                "exa".to_string(),
                                if ok {
                                    "exa-results-found".to_string()
                                } else {
                                    format!("exa-no-results; grok={e}")
                                },
                            );
                        }
                        Err(exa_err) => {
                            if let Ok(Some(brave)) =
                                get_secret_for_component("parallel-agents-core", "brave_api_key")
                            {
                                match request_brave(&q, &brave) {
                                    Ok(payload) => {
                                        let ok = payload.contains("\"web\"")
                                            || payload.contains("\"results\"");
                                        return (
                                            ok,
                                            "brave".to_string(),
                                            if ok {
                                                "brave-results-found".to_string()
                                            } else {
                                                format!("brave-no-results; grok={e}")
                                            },
                                        );
                                    }
                                    Err(brave_err) => {
                                        return (
                                            false,
                                            "none".to_string(),
                                            format!("grok={e}; exa={exa_err}; brave={brave_err}"),
                                        );
                                    }
                                }
                            }
                            return (
                                false,
                                "none".to_string(),
                                format!("grok={e}; exa={exa_err}; brave=missing-key"),
                            );
                        }
                    }
                }
            }
        }
    }
    let q = make_verify_query(prompt, response);

    if let Ok(Some(exa)) = get_secret_for_component("parallel-agents-core", "exa_api_key") {
        match request_exa(&q, &exa) {
            Ok(payload) => {
                let ok = payload.contains("\"results\"");
                return (
                    ok,
                    "exa".to_string(),
                    if ok {
                        "exa-results-found".to_string()
                    } else {
                        "exa-no-results".to_string()
                    },
                );
            }
            Err(e) => {
                if let Ok(Some(brave)) =
                    get_secret_for_component("parallel-agents-core", "brave_api_key")
                {
                    match request_brave(&q, &brave) {
                        Ok(payload) => {
                            let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                            return (
                                ok,
                                "brave".to_string(),
                                if ok {
                                    "brave-results-found".to_string()
                                } else {
                                    "brave-no-results".to_string()
                                },
                            );
                        }
                        Err(e2) => match verify_with_grok(prompt, response) {
                            Ok(result) => return result,
                            Err(grok_err) => {
                                return (
                                    false,
                                    "none".to_string(),
                                    format!("exa={e}; brave={e2}; grok={grok_err}"),
                                );
                            }
                        },
                    }
                }
                match verify_with_grok(prompt, response) {
                    Ok(result) => return result,
                    Err(grok_err) => {
                        return (
                            false,
                            "none".to_string(),
                            format!("exa={e}; brave=missing-key; grok={grok_err}"),
                        );
                    }
                }
            }
        }
    }

    if let Ok(Some(brave)) = get_secret_for_component("parallel-agents-core", "brave_api_key") {
        match request_brave(&q, &brave) {
            Ok(payload) => {
                let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                return (
                    ok,
                    "brave".to_string(),
                    if ok {
                        "brave-results-found".to_string()
                    } else {
                        "brave-no-results".to_string()
                    },
                );
            }
            Err(e) => match verify_with_grok(prompt, response) {
                Ok(result) => return result,
                Err(grok_err) => {
                    return (
                        false,
                        "none".to_string(),
                        format!("brave={e}; grok={grok_err}"),
                    );
                }
            },
        }
    }

    match verify_with_grok(prompt, response) {
        Ok(result) => result,
        Err(e) => (
            false,
            "none".to_string(),
            format!("missing-search-keys; grok={e}"),
        ),
    }
}
