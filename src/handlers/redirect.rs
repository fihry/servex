use super::HttpResponse;

pub fn handle_redirect(status: u16, target: String) -> HttpResponse {
    let reason = match status {
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        _ => "Found",
    };

    HttpResponse {
        status_line: format!("{} {}", status, reason),
        content_type: "text/plain; charset=utf-8",
        body: Vec::new(),
        extra_headers: vec![("Location".to_string(), target)],
    }
}
