//! Table and form extraction from HTML.

use super::html::{extract_attr, strip_tags};

/// Extract tables as Vec of tables, each table is Vec of rows, each row is Vec of cells.
pub fn extract_tables(html: &str) -> Vec<Vec<Vec<String>>> {
    let mut tables = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    while search_from < lower.len() {
        let table_start = match lower[search_from..].find("<table") {
            Some(p) => search_from + p,
            None => break,
        };

        let table_end = match lower[table_start..].find("</table>") {
            Some(p) => table_start + p + 8,
            None => break,
        };

        let table_html = &html[table_start..table_end];
        let table_lower = &lower[table_start..table_end];
        let mut rows = Vec::new();
        let mut row_search = 0;

        while row_search < table_lower.len() {
            let tr_start = match table_lower[row_search..].find("<tr") {
                Some(p) => row_search + p,
                None => break,
            };

            let tr_end = match table_lower[tr_start..].find("</tr>") {
                Some(p) => tr_start + p + 5,
                None => break,
            };

            let row_html = &table_html[tr_start..tr_end];
            let row_lower = table_lower[tr_start..tr_end].to_string();
            let mut cells = Vec::new();
            let mut cell_search = 0;

            while cell_search < row_lower.len() {
                let td_pos = row_lower[cell_search..].find("<td");
                let th_pos = row_lower[cell_search..].find("<th");

                let cell_start = match (td_pos, th_pos) {
                    (Some(a), Some(b)) => cell_search + a.min(b),
                    (Some(a), None) => cell_search + a,
                    (None, Some(b)) => cell_search + b,
                    (None, None) => break,
                };

                let content_start = match row_lower[cell_start..].find('>') {
                    Some(p) => cell_start + p + 1,
                    None => break,
                };

                let td_end = row_lower[content_start..].find("</td>");
                let th_end = row_lower[content_start..].find("</th>");

                let cell_end = match (td_end, th_end) {
                    (Some(a), Some(b)) => content_start + a.min(b),
                    (Some(a), None) => content_start + a,
                    (None, Some(b)) => content_start + b,
                    (None, None) => break,
                };

                let text = strip_tags(&row_html[content_start..cell_end])
                    .trim()
                    .to_string();
                cells.push(text);
                cell_search = cell_end + 5;
            }

            if !cells.is_empty() {
                rows.push(cells);
            }
            row_search = tr_end;
        }

        if !rows.is_empty() {
            tables.push(rows);
        }
        search_from = table_end;
    }

    tables
}

/// Form field information.
pub struct FormField {
    pub name: String,
    pub field_type: String,
    pub placeholder: String,
}

/// Form information.
pub struct FormInfo {
    pub action: String,
    pub method: String,
    pub fields: Vec<FormField>,
}

/// Extract form elements with their fields.
pub fn extract_forms(html: &str) -> Vec<FormInfo> {
    let mut forms = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    while search_from < lower.len() {
        let form_start = match lower[search_from..].find("<form") {
            Some(p) => search_from + p,
            None => break,
        };

        let form_end = match lower[form_start..].find("</form>") {
            Some(p) => form_start + p + 7,
            None => break,
        };

        let form_tag_end = match lower[form_start..].find('>') {
            Some(p) => form_start + p,
            None => break,
        };

        let form_open = &html[form_start..form_tag_end + 1];
        let action = extract_attr(form_open, "action").unwrap_or_default();
        let method = extract_attr(form_open, "method").unwrap_or_else(|| "GET".to_string());

        let form_body = &html[form_tag_end + 1..form_end - 7];
        let form_body_lower = form_body.to_lowercase();
        let mut fields = Vec::new();
        let mut input_search = 0;

        while input_search < form_body_lower.len() {
            let input_start = match form_body_lower[input_search..].find("<input") {
                Some(p) => input_search + p,
                None => break,
            };

            let input_end = match form_body_lower[input_start..].find('>') {
                Some(p) => input_start + p + 1,
                None => break,
            };

            let input_tag = &form_body[input_start..input_end];
            fields.push(FormField {
                name: extract_attr(input_tag, "name").unwrap_or_default(),
                field_type: extract_attr(input_tag, "type")
                    .unwrap_or_else(|| "text".to_string()),
                placeholder: extract_attr(input_tag, "placeholder").unwrap_or_default(),
            });

            input_search = input_end;
        }

        forms.push(FormInfo {
            action,
            method,
            fields,
        });
        search_from = form_end;
    }

    forms
}

/// Extract list items from ul/ol lists.
pub fn extract_lists(html: &str) -> Vec<Vec<String>> {
    let mut lists = Vec::new();
    let lower = html.to_lowercase();
    let mut search_from = 0;

    loop {
        let ul_pos = lower[search_from..].find("<ul");
        let ol_pos = lower[search_from..].find("<ol");

        let list_start = match (ul_pos, ol_pos) {
            (Some(a), Some(b)) => search_from + a.min(b),
            (Some(a), None) => search_from + a,
            (None, Some(b)) => search_from + b,
            (None, None) => break,
        };

        let is_ul = lower[list_start..].starts_with("<ul");
        let close_tag = if is_ul { "</ul>" } else { "</ol>" };

        let list_end = match lower[list_start..].find(close_tag) {
            Some(p) => list_start + p + close_tag.len(),
            None => break,
        };

        let list_html = &html[list_start..list_end];
        let list_lower = &lower[list_start..list_end];
        let mut items = Vec::new();
        let mut li_search = 0;

        while li_search < list_lower.len() {
            let li_start = match list_lower[li_search..].find("<li") {
                Some(p) => li_search + p,
                None => break,
            };

            let content_start = match list_lower[li_start..].find('>') {
                Some(p) => li_start + p + 1,
                None => break,
            };

            let content_end = match list_lower[content_start..].find("</li>") {
                Some(p) => content_start + p,
                None => list_lower.len(),
            };

            let text = strip_tags(&list_html[content_start..content_end])
                .trim()
                .to_string();
            if !text.is_empty() {
                items.push(text);
            }

            li_search = content_end + 5;
        }

        if !items.is_empty() {
            lists.push(items);
        }
        search_from = list_end;
    }

    lists
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tables_works() {
        let html = "<table><tr><td>A</td><td>B</td></tr><tr><td>C</td><td>D</td></tr></table>";
        let tables = extract_tables(html);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].len(), 2);
        assert_eq!(tables[0][0], vec!["A", "B"]);
        assert_eq!(tables[0][1], vec!["C", "D"]);
    }
}
