//! A series' other titles, from AniList (free, no key): sources name the same work differently
//! ("Junk to King: I Am A Scavenger" on MangaDex is "Became the King by Scavenging" on Drake
//! Scans), so the autopilot searches with each of them before deciding nothing else carries it.

use std::time::Duration;

use serde_json::{Value, json};

const QUERY: &str = "query($s:String){ Page(perPage:5){ media(search:$s,type:MANGA){ title{romaji english} synonyms } } }";

/// The other titles of the work AniList knows as `title`: its English and romaji titles and its
/// synonyms, in Latin script (the sources searched are English). Empty when AniList has no entry
/// that names `title` itself, so a near miss never lends its titles.
pub fn titles(title: &str) -> Result<Vec<String>, String> {
    let config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build();
    let agent = ureq::Agent::new_with_config(config);
    let body = json!({ "query": QUERY, "variables": { "s": title } }).to_string();
    let mut resp = agent
        .post("https://graphql.anilist.co")
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .send(&body)
        .map_err(|e| format!("anilist: {e}"))?;
    let text = resp.body_mut().read_to_string().map_err(|e| format!("anilist: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("anilist: {e}"))?;
    Ok(pick(&v, title))
}

fn latin(t: &str) -> bool {
    let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    !letters.is_empty() && letters.iter().filter(|c| c.is_ascii()).count() * 10 >= letters.len() * 9
}

fn pick(v: &Value, title: &str) -> Vec<String> {
    for m in v["data"]["Page"]["media"].as_array().into_iter().flatten() {
        let mut names: Vec<String> = Vec::new();
        for t in [&m["title"]["english"], &m["title"]["romaji"]] {
            if let Some(s) = t.as_str() {
                names.push(s.to_string());
            }
        }
        for s in m["synonyms"].as_array().into_iter().flatten().filter_map(|s| s.as_str()) {
            names.push(s.to_string());
        }
        if !names.iter().any(|n| leaf_proto::same_work(n, title)) {
            continue;
        }
        let mut out: Vec<String> = Vec::new();
        for n in names.into_iter().filter(|n| latin(n)) {
            let key = leaf_proto::norm_title(&n);
            if key != leaf_proto::norm_title(title) && !out.iter().any(|o| leaf_proto::norm_title(o) == key) {
                out.push(n);
            }
        }
        return out;
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_entry_that_names_the_title_lends_its_names() {
        let v = json!({ "data": { "Page": { "media": [
            { "title": { "romaji": "Other Thing", "english": "Something Else" }, "synonyms": ["Another"] },
            { "title": { "romaji": "Wo Kao Jian Lese Shang Wang Zhe", "english": "Junk to King: I Am A Scavenger" },
              "synonyms": ["I Became the King By Scavenging", "Jeu Mondial : De déchet à Roi des Abysses", "我靠捡垃圾上王者"] }
        ] } } });
        assert_eq!(
            pick(&v, "Junk to King: I Am A Scavenger"),
            vec!["Wo Kao Jian Lese Shang Wang Zhe", "I Became the King By Scavenging", "Jeu Mondial : De déchet à Roi des Abysses"]
        );
        assert!(pick(&v, "Nothing Like It").is_empty());
        assert!(latin("Jeu Mondial : De déchet à Roi des Abysses"));
        assert!(!latin("我靠捡垃圾上王者"));
    }
}
