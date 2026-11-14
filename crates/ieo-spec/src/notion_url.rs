//! The Notion page a URL names — one strict rule shared by the coordinator's import check and
//! the importer (X-02 preparation, user decision 2026-11-14 §5).

/// The page id a Notion URL names: query and fragment removed, the last path segment without
/// dashes must end in 32 hex characters; those, lower-cased, are the id. `None` otherwise.
/// Scheme and host are not checked here (a host allow-list is a separate decision).
pub fn page_id(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let seg: String = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .chars()
        .filter(|c| *c != '-')
        .collect();
    let tail = seg.get(seg.len().checked_sub(32)?..)?;
    tail.chars()
        .all(|c| c.is_ascii_hexdigit())
        .then(|| tail.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::page_id;

    #[test]
    fn notion_urls_are_read_strictly() {
        let id = "0123456789abcdef0123456789abcdef";
        let up = id.to_uppercase();
        let cases: Vec<(String, Option<&str>)> = vec![
            (format!("https://app.notion.com/p/{id}"), Some(id)),
            (
                format!("https://www.notion.so/Synthetic-Task-{id}?pvs=4"),
                Some(id),
            ),
            (format!("https://www.notion.so/Cafe-{id}"), Some(id)),
            (
                format!("https://www.notion.so/my-workspace/Task-{id}"),
                Some(id),
            ),
            (
                "https://www.notion.so/01234567-89ab-cdef-0123-456789abcdef#section".into(),
                Some(id),
            ),
            (format!("https://www.notion.so/{up}"), Some(id)),
            (format!("https://www.notion.so/{id}/"), Some(id)),
            ("https://app.notion.com/p/short".into(), None),
            (format!("https://app.notion.com/{id}/x"), None),
            (format!("https://app.notion.com/p/x?id={id}"), None),
            (format!("https://app.notion.com/p/x#{id}"), None),
            (
                "https://www.notion.so/ghijklmnopqrstuvghijklmnopqrstuv".into(),
                None,
            ),
            (format!("https://www.notion.so/{id}%2F"), None),
        ];
        for (url, want) in cases {
            assert_eq!(page_id(&url).as_deref(), want, "{url}");
        }
    }
}
