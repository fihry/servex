use crate::http::models::Status;

pub fn handle_error(status: Status) -> Response {
    let error_page = match status.code {
        400 => include_str!("../error_pages/400.html"),
        403 => include_str!("../error_pages/403.html"),
        404 => include_str!("../error_pages/404.html"),
        405 => include_str!("../error_pages/405.html"),
        413 => include_str!("../error_pages/413.html"),
        500 => include_str!("../error_pages/500.html"),
        _ => "Error",
    };

    Response::new(status)
        .with_body(error_page.as_bytes())
}